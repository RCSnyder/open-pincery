//! AC-93 (v9.1, REVIEW round 2): direct test of the wake-loop's
//! `resolve_workspace_llm` helper.
//!
//! The original AC-93 contract test (`tests/cli_provider_test.rs`)
//! drives the CLI round-trip but never exercises the resolver that
//! the wake loop calls at the start of each wake. REVIEW flagged
//! this as a Required gap (round 1, finding #3). This test closes
//! it for v9.1 — minimum loveable coverage:
//!
//! 1. Workspace with a default provider + matching active credential
//!    returns `Some(LlmClient)`.
//! 2. Workspace with NO provider rows returns `None` (caller emits
//!    `llm_provider_env_fallback`).
//! 3. Workspace with a provider whose credential is revoked returns
//!    `None`.
//!
//! The deeper "key value never appears in agent process memory"
//! check (AC-93 sub-criterion (c) — AC-71 memory-grep helper) is
//! deferred to VERIFY's live-process inspection.

mod common;

use std::sync::Arc;

use open_pincery::models::{credential, llm_provider, workspace as ws_model};
use open_pincery::runtime::llm::LlmClient;
use open_pincery::runtime::vault::Vault;
use open_pincery::runtime::wake_loop::resolve_workspace_llm;
use sqlx::PgPool;
use uuid::Uuid;

const PROVIDER_BASE: &str = "https://provider.example/api/v1";
const FALLBACK_BASE: &str = "https://fallback.example/api/v1";
const SECRET_KEY: &str = "sk-real-provider-key";

struct Ctx {
    pool: PgPool,
    vault: Arc<Vault>,
    fallback_llm: LlmClient,
    workspace_id: Uuid,
    user_id: Uuid,
}

async fn setup() -> Ctx {
    let pool = common::test_pool().await;

    // Seed a unique user + org + workspace. Using a UUID suffix keeps
    // the fixture stable even though `test_pool` already truncates.
    let suffix = Uuid::new_v4().simple().to_string();
    let user_id: (Uuid,) = sqlx::query_as(
        "INSERT INTO users (email, display_name, auth_provider, auth_subject) \
         VALUES ($1, 'tester', 'test', $2) RETURNING id",
    )
    .bind(format!("tester+{suffix}@example.com"))
    .bind(format!("test:{suffix}"))
    .fetch_one(&pool)
    .await
    .expect("seed user");

    let org = ws_model::create_organization(&pool, "TestOrg", &format!("org-{suffix}"), user_id.0)
        .await
        .expect("create org");
    let ws =
        ws_model::create_workspace(&pool, org.id, "TestWS", &format!("ws-{suffix}"), user_id.0)
            .await
            .expect("create workspace");

    let vault = Arc::new(Vault::from_base64(common::TEST_VAULT_KEY_B64).expect("vault key"));
    let fallback_llm = LlmClient::new(
        FALLBACK_BASE.to_string(),
        "fallback-key".to_string(),
        "model-x".to_string(),
        "model-x-maint".to_string(),
    );

    Ctx {
        pool,
        vault,
        fallback_llm,
        workspace_id: ws.id,
        user_id: user_id.0,
    }
}

#[tokio::test]
async fn ac93_resolver_returns_some_when_provider_and_credential_exist() {
    let ctx = setup().await;

    // Seal a credential under the workspace, persist it.
    let sealed = ctx
        .vault
        .seal(ctx.workspace_id, "openrouter_key", SECRET_KEY.as_bytes())
        .expect("seal");
    credential::create(
        &ctx.pool,
        ctx.workspace_id,
        "openrouter_key",
        &sealed.ciphertext,
        &sealed.nonce,
        ctx.user_id,
    )
    .await
    .expect("persist credential");

    // Insert provider row (auto-default — it's the first one).
    llm_provider::create(
        &ctx.pool,
        ctx.workspace_id,
        "openrouter",
        PROVIDER_BASE,
        "openrouter_key",
    )
    .await
    .expect("provider create");

    let resolved =
        resolve_workspace_llm(&ctx.pool, &ctx.vault, &ctx.fallback_llm, ctx.workspace_id).await;

    assert!(
        resolved.is_some(),
        "resolver must return Some when a default provider + active credential exist"
    );
    // The model fields are carried over from the fallback LlmClient
    // because the resolver does not currently override them.
    let client = resolved.unwrap();
    assert_eq!(client.model, "model-x");
    assert_eq!(client.maintenance_model, "model-x-maint");
}

#[tokio::test]
async fn ac93_resolver_returns_none_when_no_provider_rows() {
    let ctx = setup().await;

    let resolved =
        resolve_workspace_llm(&ctx.pool, &ctx.vault, &ctx.fallback_llm, ctx.workspace_id).await;

    assert!(
        resolved.is_none(),
        "resolver must return None when workspace has no llm_providers rows (env-var fallback path)"
    );
}

#[tokio::test]
async fn ac93_resolver_returns_none_when_credential_revoked() {
    let ctx = setup().await;

    let sealed = ctx
        .vault
        .seal(ctx.workspace_id, "openrouter_key", SECRET_KEY.as_bytes())
        .expect("seal");
    credential::create(
        &ctx.pool,
        ctx.workspace_id,
        "openrouter_key",
        &sealed.ciphertext,
        &sealed.nonce,
        ctx.user_id,
    )
    .await
    .expect("persist credential");

    llm_provider::create(
        &ctx.pool,
        ctx.workspace_id,
        "openrouter",
        PROVIDER_BASE,
        "openrouter_key",
    )
    .await
    .expect("provider create");

    credential::revoke(&ctx.pool, ctx.workspace_id, "openrouter_key")
        .await
        .expect("revoke");

    let resolved =
        resolve_workspace_llm(&ctx.pool, &ctx.vault, &ctx.fallback_llm, ctx.workspace_id).await;

    assert!(
        resolved.is_none(),
        "resolver must return None when the referenced credential has been revoked"
    );
}
