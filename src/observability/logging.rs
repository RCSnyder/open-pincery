//! AC-17: JSON vs human-readable logging toggle.
//!
//! `LOG_FORMAT=json` → one JSON object per line via `tracing-subscriber`'s
//! `json()` formatter. Any other value (or unset) → default human-readable
//! output. Log level still controlled by `RUST_LOG` via `EnvFilter`.

use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::EnvFilter;

/// Returns true if `LOG_FORMAT=json`, selecting JSON output.
pub fn is_json_format() -> bool {
    matches!(std::env::var("LOG_FORMAT").ok().as_deref(), Some("json"))
}

/// Install the global tracing subscriber exactly once.
/// Safe to call only from `main`. Honours `RUST_LOG` and `LOG_FORMAT`.
pub fn init_logging() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    if is_json_format() {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .json()
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    }
}

/// Build a JSON-format subscriber that writes to the provided writer.
/// Exposed for tests that assert the exact wire format of the JSON output
/// that `init_logging()` would emit when `LOG_FORMAT=json` is set.
///
/// The returned subscriber must be activated with `set_default` (test scope)
/// or `init` (process-wide). This function intentionally does not call either.
pub fn json_subscriber_for_writer<W>(writer: W) -> impl tracing::Subscriber + Send + Sync
where
    W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new("info"))
        .json()
        .with_writer(writer)
        .finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::fmt::MakeWriter;

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

    /// MakeWriter that appends every line to a shared `Vec<u8>` buffer so a
    /// test can parse what the JSON formatter produced.
    #[derive(Clone)]
    struct BufWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for BufWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for BufWriter {
        type Writer = BufWriter;
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    #[test]
    fn json_output_is_parseable_with_required_fields() {
        let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = BufWriter(buf.clone());
        let subscriber = json_subscriber_for_writer(writer);

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "ac17_test", "hello world");
        });

        let bytes = buf.lock().unwrap().clone();
        let text = String::from_utf8(bytes).expect("utf-8 log output");
        assert!(!text.is_empty(), "expected at least one log line");

        // Every non-empty line must be valid JSON with the standard fields.
        let mut saw_event = false;
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let v: serde_json::Value =
                serde_json::from_str(line).unwrap_or_else(|e| panic!("invalid JSON: {line}: {e}"));
            assert!(v.get("timestamp").is_some(), "missing timestamp: {line}");
            assert!(v.get("level").is_some(), "missing level: {line}");
            assert!(v.get("target").is_some(), "missing target: {line}");
            assert!(v.get("fields").is_some(), "missing fields: {line}");
            if v["target"] == "ac17_test" {
                assert_eq!(v["level"], "INFO");
                assert_eq!(v["fields"]["message"], "hello world");
                saw_event = true;
            }
        }
        assert!(
            saw_event,
            "expected to see the emitted test event in JSON output"
        );
    }
}
