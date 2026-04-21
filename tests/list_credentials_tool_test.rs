//! AC-41 (v7): `list_credentials` tool contract.
//!
//! Assertions:
//!   * Registered in `tool_definitions()`.
//!   * Classified as [`ToolCapability::ReadLocal`], so it is allowed
//!     under every permission mode including [`PermissionMode::Locked`].
//!   * Returns only names + metadata; ciphertext and plaintext values
//!     NEVER appear in the tool output, even when real sealed rows
//!     exist in the database.
//!   * Returns rows scoped to the caller's workspace only.

mod common;

use open_pincery::models::{credential, user, workspace};
use open_pincery::runtime::capability::{self, PermissionMode, ToolCapability};
use open_pincery::runtime::llm::{FunctionCall, ToolCallRequest};
use open_pincery::runtime::sandbox::{ProcessExecutor, ToolExecutor};
use open_pincery::runtime::tools::{self, ToolResult};
use open_pincery::runtime::vault::Vault;
use std::sync::Arc;
use uuid::Uuid;

#[test]
fn ac41_list_credentials_is_registered_as_read_local() {
    // Tool definition is discoverable.
    let defs = tools::tool_definitions();
    assert!(
        defs.iter().any(|d| d.function.name == "list_credentials"),
        "list_credentials missing from tool_definitions()"
    );

    // Capability classification: ReadLocal → allowed in every mode.
    assert_eq!(
        capability::required_for("list_credentials"),
        ToolCapability::ReadLocal
    );
    assert!(capability::mode_allows(
        PermissionMode::Locked,
        ToolCapability::ReadLocal
    ));
    assert!(capability::mode_allows(
        PermissionMode::Supervised,
        ToolCapability::ReadLocal
    ));
    assert!(capability::mode_allows(
        PermissionMode::Yolo,
        ToolCapability::ReadLocal
    ));
}

#[tokio::test]
async fn ac41_list_credentials_returns_names_only_and_scoped_to_workspace() {
    let pool = common::test_pool().await;
    let vault = Vault::from_base64(common::TEST_VAULT_KEY_B64).unwrap();

    // Set up two workspaces within one org to confirm per-workspace scope.
    let u = user::create_local_admin(&pool, "admin@test.local", "Admin")
        .await
        .unwrap();
    let org = workspace::create_organization(&pool, "org", "org", u.id)
        .await
        .unwrap();
    let ws_a = workspace::create_workspace(&pool, org.id, "ws-a", "ws-a", u.id)
        .await
        .unwrap();
    let ws_b = workspace::create_workspace(&pool, org.id, "ws-b", "ws-b", u.id)
        .await
        .unwrap();

    let secret_a = b"SECRET_ONLY_IN_WS_A_SHOULD_NEVER_LEAK";
    let secret_b = b"SECRET_ONLY_IN_WS_B_SHOULD_NEVER_LEAK";

    let sealed_a = vault.seal(ws_a.id, "alpha", secret_a).unwrap();
    let sealed_b = vault.seal(ws_b.id, "beta", secret_b).unwrap();

    credential::create(
        &pool,
        ws_a.id,
        "alpha",
        &sealed_a.ciphertext,
        &sealed_a.nonce,
        u.id,
    )
    .await
    .unwrap();
    credential::create(
        &pool,
        ws_b.id,
        "beta",
        &sealed_b.ciphertext,
        &sealed_b.nonce,
        u.id,
    )
    .await
    .unwrap();

    // Dispatch the tool as an agent in ws_a.
    let tc = ToolCallRequest {
        id: "call-1".into(),
        call_type: "function".into(),
        function: FunctionCall {
            name: "list_credentials".into(),
            arguments: "{}".into(),
        },
    };
    let executor: Arc<dyn ToolExecutor> = Arc::new(ProcessExecutor);
    let vault_arc = Arc::new(vault);
    let result = tools::dispatch_tool(
        &tc,
        PermissionMode::Locked, // ReadLocal must work even in Locked
        &pool,
        Uuid::new_v4(), // agent_id — not actually used by this tool
        ws_a.id,
        Uuid::new_v4(), // wake_id — not actually used by this tool
        &executor,
        &vault_arc,
    )
    .await;

    let body = match result {
        ToolResult::Output(s) => s,
        ToolResult::Sleep => panic!("expected Output, got Sleep"),
        ToolResult::Error(e) => panic!("expected Output, got Error: {e}"),
    };

    // --- Scope: ws_a row present, ws_b row absent.
    assert!(
        body.contains("\"alpha\""),
        "expected 'alpha' in body: {body}"
    );
    assert!(
        !body.contains("\"beta\""),
        "cross-workspace leak — 'beta' appeared in ws_a output: {body}"
    );

    // --- Secrecy: neither plaintext nor sealed ciphertext leaves the DB.
    assert!(
        !body.contains("SECRET_ONLY_IN_WS_A_SHOULD_NEVER_LEAK"),
        "plaintext leaked into tool output"
    );
    assert!(!body.contains("ciphertext"));
    assert!(!body.contains("nonce"));

    // --- Shape: outer `credentials` array with objects containing `name`.
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    let arr = parsed["credentials"].as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["name"].as_str().unwrap(), "alpha");
    assert!(arr[0]["value"].is_null());
}
