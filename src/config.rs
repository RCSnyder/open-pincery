use std::net::SocketAddr;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub host: String,
    pub port: u16,
    pub bootstrap_token: String,
    pub llm_api_base_url: String,
    pub llm_api_key: String,
    pub llm_model: String,
    pub llm_maintenance_model: String,
    pub max_prompt_chars: usize,
    pub iteration_cap: i32,
    pub stale_wake_hours: i64,
    pub wake_summary_limit: i64,
    pub event_window_limit: i64,
    /// AC-38 (v7): base64-encoded 32-byte AES-256-GCM master key for the
    /// credential vault. Loaded from `OPEN_PINCERY_VAULT_KEY`. Validated
    /// at startup by `Vault::from_base64` — this field is kept as the raw
    /// base64 string so tests can build a `Config` without decoding
    /// first; the decoded `Vault` lives on `AppState`.
    pub vault_key_b64: String,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            database_url: require_env("DATABASE_URL")?,
            host: env_or("OPEN_PINCERY_HOST", "0.0.0.0"),
            port: env_or("OPEN_PINCERY_PORT", "8080")
                .parse()
                .map_err(|e| format!("Invalid OPEN_PINCERY_PORT: {e}"))?,
            bootstrap_token: require_env("OPEN_PINCERY_BOOTSTRAP_TOKEN")?,
            llm_api_base_url: require_env("LLM_API_BASE_URL")?,
            llm_api_key: require_env("LLM_API_KEY")?,
            llm_model: env_or("LLM_MODEL", "anthropic/claude-sonnet-4-20250514"),
            llm_maintenance_model: env_or(
                "LLM_MAINTENANCE_MODEL",
                "anthropic/claude-sonnet-4-20250514",
            ),
            max_prompt_chars: env_or("MAX_PROMPT_CHARS", "100000")
                .parse()
                .map_err(|e| format!("Invalid MAX_PROMPT_CHARS: {e}"))?,
            iteration_cap: env_or("ITERATION_CAP", "50")
                .parse()
                .map_err(|e| format!("Invalid ITERATION_CAP: {e}"))?,
            stale_wake_hours: env_or("STALE_WAKE_HOURS", "2")
                .parse()
                .map_err(|e| format!("Invalid STALE_WAKE_HOURS: {e}"))?,
            wake_summary_limit: env_or("WAKE_SUMMARY_LIMIT", "20")
                .parse()
                .map_err(|e| format!("Invalid WAKE_SUMMARY_LIMIT: {e}"))?,
            event_window_limit: env_or("EVENT_WINDOW_LIMIT", "200")
                .parse()
                .map_err(|e| format!("Invalid EVENT_WINDOW_LIMIT: {e}"))?,
            vault_key_b64: require_env("OPEN_PINCERY_VAULT_KEY")?,
        })
    }

    pub fn socket_addr(&self) -> SocketAddr {
        format!("{}:{}", self.host, self.port)
            .parse()
            .expect("Invalid socket address")
    }
}

fn require_env(key: &str) -> Result<String, String> {
    std::env::var(key).map_err(|_| format!("Missing required environment variable: {key}"))
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
