//! AC-17: JSON vs human-readable logging toggle.
//!
//! `LOG_FORMAT=json` → one JSON object per line via `tracing-subscriber`'s
//! `json()` formatter. Any other value (or unset) → default human-readable
//! output. Log level still controlled by `RUST_LOG` via `EnvFilter`.

use tracing_subscriber::EnvFilter;

/// Returns true if `LOG_FORMAT=json`, selecting JSON output.
pub fn is_json_format() -> bool {
    matches!(std::env::var("LOG_FORMAT").ok().as_deref(), Some("json"))
}

/// Install the global tracing subscriber exactly once.
/// Safe to call only from `main`. Honours `RUST_LOG` and `LOG_FORMAT`.
pub fn init_logging() {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    if is_json_format() {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .json()
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .init();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_json_format_true_when_env_set() {
        // Safety: single-threaded env mutation inside test; no other test reads LOG_FORMAT.
        std::env::set_var("LOG_FORMAT", "json");
        assert!(is_json_format());
        std::env::remove_var("LOG_FORMAT");
    }

    #[test]
    fn is_json_format_false_when_unset() {
        std::env::remove_var("LOG_FORMAT");
        assert!(!is_json_format());
    }

    #[test]
    fn is_json_format_false_for_other_values() {
        std::env::set_var("LOG_FORMAT", "pretty");
        assert!(!is_json_format());
        std::env::remove_var("LOG_FORMAT");
    }
}
