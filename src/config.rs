use std::net::SocketAddr;

/// AC-73 (v9): Sandbox enforcement mode.
///
/// `Enforce` is the default and the only safe production value. `Audit`
/// runs the policy engine but logs `sandbox_would_block` events instead
/// of actually blocking — used by operators to shake out allowlists
/// during upgrade. `Disabled` bypasses the sandbox entirely and is only
/// accepted when paired with `OPEN_PINCERY_ALLOW_UNSAFE=true`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SandboxMode {
    Enforce,
    Audit,
    Disabled,
}

impl SandboxMode {
    /// Case-insensitive parser. The canonical lowercase form is what
    /// `.env.example` documents and what `Display` emits.
    pub fn parse(s: &str) -> Result<Self, SandboxModeError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "enforce" => Ok(Self::Enforce),
            "audit" => Ok(Self::Audit),
            "disabled" => Ok(Self::Disabled),
            other => Err(SandboxModeError::Invalid(other.to_string())),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enforce => "enforce",
            Self::Audit => "audit",
            Self::Disabled => "disabled",
        }
    }
}

impl std::fmt::Display for SandboxMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Errors surfaced while resolving the sandbox-mode env pair.
#[derive(Debug, PartialEq, Eq)]
pub enum SandboxModeError {
    /// `OPEN_PINCERY_SANDBOX_MODE` held an unrecognised value.
    Invalid(String),
    /// `disabled` was requested without the paired
    /// `OPEN_PINCERY_ALLOW_UNSAFE=true` opt-in (AC-73 footgun guard).
    DisabledRequiresAllowUnsafe,
}

impl std::fmt::Display for SandboxModeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(v) => write!(
                f,
                "OPEN_PINCERY_SANDBOX_MODE={v:?} is not one of: enforce | audit | disabled"
            ),
            Self::DisabledRequiresAllowUnsafe => f.write_str(
                "OPEN_PINCERY_SANDBOX_MODE=disabled requires the paired \
                 OPEN_PINCERY_ALLOW_UNSAFE=true opt-in (refusing to run)",
            ),
        }
    }
}

impl std::error::Error for SandboxModeError {}

/// The validated sandbox-mode configuration as resolved from two env
/// variables. `Config::from_env()` stores a clone of this on itself;
/// tests drive the pure [`ResolvedSandboxMode::resolve`] function
/// directly with explicit values so they never have to race on
/// `std::env::set_var`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedSandboxMode {
    pub mode: SandboxMode,
    /// Mirrors the raw `OPEN_PINCERY_ALLOW_UNSAFE=true` flag. Kept on
    /// the struct (not just consumed for validation) so the startup
    /// warning path in Slice A2b can surface "unsafe mode is armed but
    /// sandbox is still enforcing" as a distinct state.
    pub allow_unsafe: bool,
}

impl ResolvedSandboxMode {
    /// Pure resolver. Accepts `Option<&str>` for the two env values so
    /// callers can pass `std::env::var(...).ok().as_deref()` in
    /// production and literals in tests.
    pub fn resolve(
        mode: Option<&str>,
        allow_unsafe: Option<&str>,
    ) -> Result<Self, SandboxModeError> {
        let mode = match mode {
            None => SandboxMode::Enforce,
            Some(raw) => SandboxMode::parse(raw)?,
        };
        let allow_unsafe = matches!(allow_unsafe, Some(v) if v.trim().eq_ignore_ascii_case("true"));
        if matches!(mode, SandboxMode::Disabled) && !allow_unsafe {
            return Err(SandboxModeError::DisabledRequiresAllowUnsafe);
        }
        Ok(Self { mode, allow_unsafe })
    }
}

impl Default for ResolvedSandboxMode {
    fn default() -> Self {
        Self {
            mode: SandboxMode::Enforce,
            allow_unsafe: false,
        }
    }
}

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
    /// AC-79 (v9 Phase G G4d): per-wake cap on consecutive
    /// schema-invalid LLM tool-call responses. After this many
    /// schema-invalid responses in one wake, the wake terminates with
    /// `termination_reason = "FailureAuditPending"` and the agent's
    /// status flips so an operator must triage. Default 3, env override
    /// `OPEN_PINCERY_SCHEMA_INVALID_RETRY_CAP`. A value of 0 is rejected
    /// at startup — operators may tighten the cap but not silently
    /// disable schema validation. Schema-invalid retries do NOT count
    /// against `iteration_cap` (so a misbehaving model cannot starve a
    /// well-behaved retry).
    pub schema_invalid_retry_cap: u32,
    pub stale_wake_hours: i64,
    pub wake_summary_limit: i64,
    pub event_window_limit: i64,
    /// AC-38 (v7): base64-encoded 32-byte AES-256-GCM master key for the
    /// credential vault. Loaded from `OPEN_PINCERY_VAULT_KEY`. Validated
    /// at startup by `Vault::from_base64` — this field is kept as the raw
    /// base64 string so tests can build a `Config` without decoding
    /// first; the decoded `Vault` lives on `AppState`.
    pub vault_key_b64: String,
    /// AC-73 (v9): sandbox enforcement mode + unsafe-opt-in pair. The
    /// actual sandbox module (AC-53) lands in Slice A2b; this field is
    /// the plumbing that every later slice reads to decide whether to
    /// block, log-only, or pass-through.
    pub sandbox: ResolvedSandboxMode,
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
            schema_invalid_retry_cap: {
                let raw = env_or("OPEN_PINCERY_SCHEMA_INVALID_RETRY_CAP", "3");
                let v: u32 = raw.parse().map_err(|e| {
                    format!("Invalid OPEN_PINCERY_SCHEMA_INVALID_RETRY_CAP={raw}: {e}")
                })?;
                if v == 0 {
                    return Err(
                        "OPEN_PINCERY_SCHEMA_INVALID_RETRY_CAP=0 is rejected: operators may tighten the schema-invalid retry cap but not silently disable AC-79 validation; minimum 1".into(),
                    );
                }
                v
            },
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
            sandbox: ResolvedSandboxMode::resolve(
                std::env::var("OPEN_PINCERY_SANDBOX_MODE").ok().as_deref(),
                std::env::var("OPEN_PINCERY_ALLOW_UNSAFE").ok().as_deref(),
            )
            .map_err(|e| e.to_string())?,
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
