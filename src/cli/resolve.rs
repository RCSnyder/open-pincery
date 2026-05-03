//! AC-46 (v8): name-or-UUID resolver for CLI noun arguments.
//!
//! The v7 CLI only accepted UUIDs. v8 lets operators type
//! `pcy agent get my-bot` and have the CLI resolve `my-bot` to the
//! matching agent's UUID via a single list-and-filter call.
//!
//! Rules — deliberately conservative (see scope.md AC-46 and the
//! "resolver only handles UUIDs" scope-reduction risk in
//! readiness.md):
//!
//! 1. Input that parses as a `Uuid` is returned verbatim. No list call.
//! 2. Otherwise, call `list_*` and filter for **exact** `name == input`.
//!    Substring and case-insensitive matching are explicitly forbidden.
//! 3. Zero matches → [`ResolveError::NotFound`] (CLI exits 1).
//! 4. Two or more matches → [`ResolveError::Ambiguous`] with the full
//!    candidate list so the caller can render a disambiguation table
//!    on stderr (CLI exits 2).
//!
//! The resolver is [`ApiClient`]-agnostic at the input layer: callers
//! pass the already-fetched `serde_json::Value` list, which keeps this
//! module trivially unit-testable without a running server.

use serde_json::Value;
use uuid::Uuid;

use crate::error::AppError;

/// A minimal `(id, name)` record used in ambiguity reporting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedRecord {
    pub id: String,
    pub name: String,
}

/// Errors specific to resolution. Converted to exit codes by the
/// top-level CLI entry point — `NotFound` is `1`, `Ambiguous` is `2`.
#[derive(Debug)]
pub enum ResolveError {
    NotFound {
        noun: &'static str,
        input: String,
    },
    Ambiguous {
        noun: &'static str,
        input: String,
        candidates: Vec<NamedRecord>,
    },
    /// The listing response did not match the expected shape — typically
    /// a server contract bug. Converted to a 500-class CLI error.
    MalformedList {
        noun: &'static str,
        reason: String,
    },
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { noun, input } => {
                write!(f, "{noun} {input:?} not found")
            }
            Self::Ambiguous {
                noun,
                input,
                candidates,
            } => {
                write!(
                    f,
                    "{} {:?} is ambiguous; {} candidates",
                    noun,
                    input,
                    candidates.len()
                )
            }
            Self::MalformedList { noun, reason } => {
                write!(f, "{noun} list response malformed: {reason}")
            }
        }
    }
}

impl std::error::Error for ResolveError {}

impl From<ResolveError> for AppError {
    fn from(e: ResolveError) -> Self {
        match &e {
            ResolveError::NotFound { .. } | ResolveError::Ambiguous { .. } => {
                AppError::BadRequest(e.to_string())
            }
            ResolveError::MalformedList { .. } => AppError::Internal(e.to_string()),
        }
    }
}

/// Core resolver: given an already-fetched array of `{id, name, ...}`
/// objects, resolve `input` to a single id. `input` may be a UUID
/// (returned verbatim, no match required against the list) or a
/// plain name string (exact-equality match only).
///
/// `noun` is a static label used in error messages (`"agent"`,
/// `"credential"`, …); it does not affect logic.
pub fn resolve_id_from_list(
    noun: &'static str,
    input: &str,
    list: &Value,
) -> Result<String, ResolveError> {
    // UUID path — short-circuit. We accept the v7 bare-UUID input
    // without a round-trip so `pcy agent get <uuid>` stays a single
    // HTTP call.
    if Uuid::parse_str(input).is_ok() {
        return Ok(input.to_string());
    }

    let records = parse_named_list(noun, list)?;
    let matches: Vec<&NamedRecord> = records.iter().filter(|r| r.name == input).collect();
    match matches.len() {
        0 => Err(ResolveError::NotFound {
            noun,
            input: input.to_string(),
        }),
        1 => Ok(matches[0].id.clone()),
        _ => Err(ResolveError::Ambiguous {
            noun,
            input: input.to_string(),
            candidates: matches.iter().map(|r| (*r).clone()).collect(),
        }),
    }
}

/// Parse the server list response shape — an array of objects with
/// at minimum `id: string` and `name: string` fields — into a typed
/// slice. Extra fields are ignored.
fn parse_named_list(noun: &'static str, list: &Value) -> Result<Vec<NamedRecord>, ResolveError> {
    let arr = list.as_array().ok_or_else(|| ResolveError::MalformedList {
        noun,
        reason: format!("expected array, got {}", value_kind(list)),
    })?;
    let mut out = Vec::with_capacity(arr.len());
    for (idx, item) in arr.iter().enumerate() {
        let obj = item
            .as_object()
            .ok_or_else(|| ResolveError::MalformedList {
                noun,
                reason: format!("item[{idx}] is not an object"),
            })?;
        let id = obj
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| ResolveError::MalformedList {
                noun,
                reason: format!("item[{idx}] missing string `id`"),
            })?
            .to_string();
        let name = obj
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| ResolveError::MalformedList {
                noun,
                reason: format!("item[{idx}] missing string `name`"),
            })?
            .to_string();
        out.push(NamedRecord { id, name });
    }
    Ok(out)
}

fn value_kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn agents_fixture() -> Value {
        json!([
            { "id": "018f0000-0000-7000-8000-000000000001", "name": "alpha" },
            { "id": "018f0000-0000-7000-8000-000000000002", "name": "beta" },
            { "id": "018f0000-0000-7000-8000-000000000003", "name": "alpha" },
        ])
    }

    #[test]
    fn uuid_input_is_returned_verbatim_without_list_lookup() {
        let uuid = "018f0000-0000-7000-8000-000000000999";
        // Empty list proves the UUID path does not consult it.
        let got = resolve_id_from_list("agent", uuid, &json!([])).unwrap();
        assert_eq!(got, uuid);
    }

    #[test]
    fn exact_name_resolves_to_id() {
        let list = agents_fixture();
        let got = resolve_id_from_list("agent", "beta", &list).unwrap();
        assert_eq!(got, "018f0000-0000-7000-8000-000000000002");
    }

    #[test]
    fn missing_name_is_not_found() {
        let list = agents_fixture();
        let err = resolve_id_from_list("agent", "gamma", &list).unwrap_err();
        match err {
            ResolveError::NotFound { noun, input } => {
                assert_eq!(noun, "agent");
                assert_eq!(input, "gamma");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn duplicate_names_are_ambiguous_with_both_candidates() {
        let list = agents_fixture();
        let err = resolve_id_from_list("agent", "alpha", &list).unwrap_err();
        match err {
            ResolveError::Ambiguous {
                noun,
                input,
                candidates,
            } => {
                assert_eq!(noun, "agent");
                assert_eq!(input, "alpha");
                assert_eq!(candidates.len(), 2);
                assert!(candidates.iter().all(|c| c.name == "alpha"));
                assert_eq!(candidates[0].id, "018f0000-0000-7000-8000-000000000001");
                assert_eq!(candidates[1].id, "018f0000-0000-7000-8000-000000000003");
            }
            other => panic!("expected Ambiguous, got {other:?}"),
        }
    }

    #[test]
    fn substring_match_is_explicitly_unsupported() {
        let list = agents_fixture();
        // Guardrails scope-lock: partial/fuzzy matches must be NotFound.
        let err = resolve_id_from_list("agent", "alph", &list).unwrap_err();
        assert!(matches!(err, ResolveError::NotFound { .. }));
    }

    #[test]
    fn case_mismatch_is_not_a_match() {
        let list = agents_fixture();
        let err = resolve_id_from_list("agent", "ALPHA", &list).unwrap_err();
        assert!(matches!(err, ResolveError::NotFound { .. }));
    }

    #[test]
    fn non_array_response_is_malformed() {
        let err = resolve_id_from_list("agent", "alpha", &json!({"wrong": true})).unwrap_err();
        assert!(matches!(err, ResolveError::MalformedList { .. }));
    }

    #[test]
    fn item_missing_name_is_malformed() {
        let list = json!([{ "id": "018f0000-0000-7000-8000-000000000001" }]);
        let err = resolve_id_from_list("agent", "alpha", &list).unwrap_err();
        assert!(matches!(err, ResolveError::MalformedList { .. }));
    }

    #[test]
    fn resolve_error_maps_to_app_error_with_correct_class() {
        let nf = ResolveError::NotFound {
            noun: "agent",
            input: "x".into(),
        };
        let app: AppError = nf.into();
        assert!(matches!(app, AppError::BadRequest(_)));

        let mal = ResolveError::MalformedList {
            noun: "agent",
            reason: "e".into(),
        };
        let app: AppError = mal.into();
        assert!(matches!(app, AppError::Internal(_)));
    }
}
