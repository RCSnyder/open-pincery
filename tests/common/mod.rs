use sqlx::PgPool;

/// Create a test database pool. Requires TEST_DATABASE_URL env var.
pub async fn test_pool() -> PgPool {
    let url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://open_pincery:open_pincery@localhost:5432/open_pincery_test".into());

    let pool = PgPool::connect(&url)
        .await
        .expect("Failed to connect to test database");

    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    // Serialize test setup to avoid parallel TRUNCATE/INSERT races
    sqlx::query("SELECT pg_advisory_lock(42)")
        .execute(&pool)
        .await
        .expect("Failed to acquire advisory lock");

    // Clean all tables for a fresh test
    sqlx::query(
        "TRUNCATE webhook_dedup, auth_audit, tool_audit, llm_call_prompts, llm_calls, wake_summaries, 
         agent_projections, events, agents, workspace_memberships, organization_memberships,
         workspaces, organizations, user_sessions, users, prompt_templates CASCADE"
    )
    .execute(&pool)
    .await
    .expect("Failed to truncate tables");

    // Re-seed prompt templates (upsert to handle parallel test races)
    sqlx::query(
        "INSERT INTO prompt_templates (name, version, template, is_active) VALUES
         ('wake_system_prompt', 1, 'You are an AI agent.', TRUE),
         ('maintenance_prompt', 1, 'Output JSON with identity, work_list, summary.', TRUE)
         ON CONFLICT (name, version) DO UPDATE SET template = EXCLUDED.template, is_active = EXCLUDED.is_active"
    )
    .execute(&pool)
    .await
    .expect("Failed to seed templates");

    sqlx::query("SELECT pg_advisory_unlock(42)")
        .execute(&pool)
        .await
        .expect("Failed to release advisory lock");

    pool
}
