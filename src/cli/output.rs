//! AC-47 (v8): universal `--output` flag.
//!
//! One renderer layer for every data-printing CLI leaf. The `pcy`
//! binary parses `--output` on the root `Cli`, passes an
//! [`OutputFormat`] down to each noun's action, and the action
//! renders its typed result via [`render`] (or directly on a
//! [`TableRow`] slice).
//!
//! The five formats required by scope.md AC-47:
//!
//! | Variant         | Use                                                          |
//! | --------------- | ------------------------------------------------------------ |
//! | `Json`          | pretty-printed JSON — always machine-parseable, non-terminal default |
//! | `Yaml`          | YAML 1.2 via `serde_yaml`                                   |
//! | `Name`          | one `name` (or `id`) per line — grep/pipe friendly          |
//! | `Table`         | column-aligned human text — TTY default, NO_COLOR honoured  |
//! | `JsonPath(..)`  | kubectl-compatible subset via `jsonpath-rust`               |
//!
//! Every public leaf must expose `--output`. The AC-52b naming lint
//! walks the clap `Command` tree and fails closed if a data-printing
//! leaf forgets it.
//!
//! TTY default selection (see [`default_for_tty`]) is explicit rather
//! than implicit: when stdout is a TTY we render `Table`; otherwise
//! `Json`. `NO_COLOR=1` / `NO_COLOR` set-non-empty suppresses ANSI
//! colour in `Table`. `PCY_NO_TTY=1` forces non-TTY behaviour (used
//! by the test suite so it is independent of the test runner's
//! stdout handling).

use std::fmt::Write as _;
use std::io::IsTerminal;
use std::str::FromStr;

use serde::Serialize;

use crate::error::AppError;

/// Output format selected via `--output`. Parsed by clap via the
/// [`FromStr`] impl below so that `--output jsonpath='{.items[*].id}'`
/// works without a second flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Yaml,
    Name,
    Table,
    /// kubectl-compatible subset: `.foo`, `.items[*].name`,
    /// `.items[0]`, `[?(@.k==v)]`. Full JQ is intentionally out of
    /// scope; reach for `-o json | jq` if you need it.
    JsonPath(String),
}

impl Default for OutputFormat {
    fn default() -> Self {
        // The default *at parse time* is `Table`; [`default_for_tty`]
        // folds this back to `Json` when stdout is not a terminal.
        // Consumers typically call `default_for_tty(None)` rather than
        // `OutputFormat::default()` directly.
        Self::Table
    }
}

impl FromStr for OutputFormat {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Accept `jsonpath='...'`, `jsonpath=...`, and plain
        // `jsonpath` (empty expression, rejected at render time).
        if let Some(rest) = s
            .strip_prefix("jsonpath=")
            .or_else(|| s.strip_prefix("jsonpath:"))
        {
            let expr = rest.trim_matches(|c| c == '\'' || c == '"').to_string();
            if expr.is_empty() {
                return Err(AppError::BadRequest(
                    "--output jsonpath= requires a non-empty expression, e.g. \
                     --output jsonpath='{.items[*].name}'"
                        .into(),
                ));
            }
            return Ok(Self::JsonPath(expr));
        }
        match s {
            "json" => Ok(Self::Json),
            "yaml" | "yml" => Ok(Self::Yaml),
            "name" => Ok(Self::Name),
            "table" => Ok(Self::Table),
            other => Err(AppError::BadRequest(format!(
                "unknown --output {other:?}; expected one of: json, yaml, name, table, jsonpath='<expr>'"
            ))),
        }
    }
}

/// Fold a caller-supplied [`OutputFormat`] against the TTY state of
/// stdout. `None` means "no flag was passed" — pick the default: TTY
/// → `Table`, non-TTY → `Json`. `Some(fmt)` passes through verbatim.
///
/// `PCY_NO_TTY=1` forces the non-TTY branch so integration tests can
/// assert the pipe default regardless of where they are spawned.
pub fn default_for_tty(flag: Option<OutputFormat>) -> OutputFormat {
    if let Some(f) = flag {
        return f;
    }
    let forced_non_tty = std::env::var("PCY_NO_TTY")
        .map(|v| !v.is_empty() && v != "0")
        .unwrap_or(false);
    if !forced_non_tty && std::io::stdout().is_terminal() {
        OutputFormat::Table
    } else {
        OutputFormat::Json
    }
}

/// `true` when ANSI colour output should be suppressed in `Table`.
/// Any non-empty `NO_COLOR` value suppresses colour (per the
/// [no-color.org](https://no-color.org) informal spec).
pub fn no_color() -> bool {
    std::env::var("NO_COLOR")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
}

/// Typed-row contract for the `Table` renderer. Each noun lists the
/// columns its table exposes (order matters) and returns a row of
/// cells in the same order for itself. Keep this trait object-unsafe;
/// [`render`] takes `&[T]` where `T: TableRow + Serialize`.
pub trait TableRow {
    fn headers() -> &'static [&'static str];
    fn row(&self) -> Vec<String>;
}

/// Entry point: render a typed slice to a string according to the
/// selected format. Every noun action funnels through this function
/// or calls [`render_value`] / [`render_name_list`] directly.
pub fn render<T>(rows: &[T], fmt: &OutputFormat) -> Result<String, AppError>
where
    T: TableRow + Serialize,
{
    match fmt {
        OutputFormat::Json => render_json(rows),
        OutputFormat::Yaml => render_yaml(rows),
        OutputFormat::Name => Ok(render_name_rows(rows)),
        OutputFormat::Table => Ok(render_table(rows)),
        OutputFormat::JsonPath(expr) => render_jsonpath(rows, expr),
    }
}

/// Render a single (non-list) value. Used for `pcy whoami`,
/// `pcy agent get`, etc., where a one-row table is surprising.
pub fn render_value<T: Serialize>(value: &T, fmt: &OutputFormat) -> Result<String, AppError> {
    match fmt {
        OutputFormat::Json => render_json(value),
        OutputFormat::Yaml => render_yaml(value),
        OutputFormat::Name => {
            // Best-effort: serialize, look for "name" or "id"
            let v = serde_json::to_value(value)
                .map_err(|e| AppError::Internal(format!("serialize: {e}")))?;
            Ok(extract_name(&v).unwrap_or_default())
        }
        OutputFormat::Table => {
            // Single-row "table" is just `key  value` pairs.
            render_kv_table(value)
        }
        OutputFormat::JsonPath(expr) => render_jsonpath(value, expr),
    }
}

fn render_json<T: Serialize + ?Sized>(value: &T) -> Result<String, AppError> {
    serde_json::to_string_pretty(value)
        .map(|mut s| {
            s.push('\n');
            s
        })
        .map_err(|e| AppError::Internal(format!("json encode: {e}")))
}

fn render_yaml<T: Serialize + ?Sized>(value: &T) -> Result<String, AppError> {
    serde_yaml::to_string(value).map_err(|e| AppError::Internal(format!("yaml encode: {e}")))
}

fn render_name_rows<T: Serialize>(rows: &[T]) -> String {
    let mut out = String::new();
    for r in rows {
        let v = match serde_json::to_value(r) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(name) = extract_name(&v) {
            let _ = writeln!(out, "{name}");
        }
    }
    out
}

fn extract_name(v: &serde_json::Value) -> Option<String> {
    v.get("name")
        .and_then(|n| n.as_str())
        .map(String::from)
        .or_else(|| v.get("id").and_then(|n| n.as_str()).map(String::from))
}

fn render_table<T: TableRow + Serialize>(rows: &[T]) -> String {
    // We use the `tabled` crate's builder API so we do not have to
    // emit a derive per row type — each noun controls its column list
    // via `TableRow::headers()` and `TableRow::row()`.
    let mut builder = tabled::builder::Builder::default();
    builder.push_record(T::headers().iter().map(|s| s.to_string()));
    for r in rows {
        builder.push_record(r.row());
    }
    let mut table = builder.build();
    // Simple ASCII style — keeps parsing straightforward in demos and
    // avoids loading a colour dependency. NO_COLOR still short-circuits
    // the style just in case a future style introduces ANSI escapes.
    if no_color() {
        table.with(tabled::settings::Style::ascii());
    } else {
        table.with(tabled::settings::Style::modern());
    }
    let mut s = table.to_string();
    s.push('\n');
    s
}

fn render_kv_table<T: Serialize>(value: &T) -> Result<String, AppError> {
    let v =
        serde_json::to_value(value).map_err(|e| AppError::Internal(format!("serialize: {e}")))?;
    let obj = v.as_object().ok_or_else(|| {
        AppError::BadRequest("table output for non-object value is not supported".into())
    })?;
    let mut builder = tabled::builder::Builder::default();
    builder.push_record(["KEY", "VALUE"]);
    for (k, val) in obj {
        let cell = match val {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        builder.push_record([k.clone(), cell]);
    }
    let mut table = builder.build();
    if no_color() {
        table.with(tabled::settings::Style::ascii());
    } else {
        table.with(tabled::settings::Style::modern());
    }
    let mut s = table.to_string();
    s.push('\n');
    Ok(s)
}

fn render_jsonpath<T: Serialize + ?Sized>(value: &T, expr: &str) -> Result<String, AppError> {
    use jsonpath_rust::JsonPathQuery;
    let v =
        serde_json::to_value(value).map_err(|e| AppError::Internal(format!("serialize: {e}")))?;
    // `jsonpath-rust` accepts the kubectl `{.items[*].name}` shape
    // when the braces are stripped; the bare `$.items[*].name` form
    // is its native syntax. Normalise both so operators can paste
    // kubectl-style expressions verbatim.
    let normalised = normalise_jsonpath(expr);
    let result = v
        .path(&normalised)
        .map_err(|e| AppError::BadRequest(format!("--output jsonpath {expr:?}: {e}")))?;
    // `path` returns a JSON array of matches. One value per line:
    // strings are unquoted, everything else is JSON-encoded.
    // This matches `kubectl -o jsonpath` behaviour.
    let items: Vec<serde_json::Value> = match result {
        serde_json::Value::Array(a) => a,
        other => vec![other],
    };
    let mut out = String::new();
    for item in items {
        match item {
            serde_json::Value::String(s) => {
                let _ = writeln!(out, "{s}");
            }
            other => {
                let encoded = serde_json::to_string(&other)
                    .map_err(|e| AppError::Internal(format!("json encode: {e}")))?;
                let _ = writeln!(out, "{encoded}");
            }
        }
    }
    Ok(out)
}

fn normalise_jsonpath(expr: &str) -> String {
    let trimmed = expr.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        let inner = &trimmed[1..trimmed.len() - 1];
        if inner.starts_with('.') {
            return format!("${inner}");
        }
        return inner.to_string();
    }
    if trimmed.starts_with('$') {
        return trimmed.to_string();
    }
    if trimmed.starts_with('.') {
        return format!("${trimmed}");
    }
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct Row {
        id: String,
        name: String,
    }

    impl TableRow for Row {
        fn headers() -> &'static [&'static str] {
            &["ID", "NAME"]
        }
        fn row(&self) -> Vec<String> {
            vec![self.id.clone(), self.name.clone()]
        }
    }

    fn fixture() -> Vec<Row> {
        vec![
            Row {
                id: "a1".into(),
                name: "alpha".into(),
            },
            Row {
                id: "b2".into(),
                name: "beta".into(),
            },
        ]
    }

    #[test]
    fn from_str_accepts_core_variants() {
        assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
        assert_eq!("yaml".parse::<OutputFormat>().unwrap(), OutputFormat::Yaml);
        assert_eq!("yml".parse::<OutputFormat>().unwrap(), OutputFormat::Yaml);
        assert_eq!("name".parse::<OutputFormat>().unwrap(), OutputFormat::Name);
        assert_eq!(
            "table".parse::<OutputFormat>().unwrap(),
            OutputFormat::Table
        );
    }

    #[test]
    fn from_str_accepts_jsonpath_quoted_and_unquoted() {
        let q = "jsonpath='{.items[*].name}'"
            .parse::<OutputFormat>()
            .unwrap();
        let u = "jsonpath={.items[*].name}".parse::<OutputFormat>().unwrap();
        assert_eq!(q, OutputFormat::JsonPath("{.items[*].name}".into()));
        assert_eq!(u, OutputFormat::JsonPath("{.items[*].name}".into()));
    }

    #[test]
    fn from_str_rejects_empty_jsonpath_expression() {
        let err = "jsonpath=".parse::<OutputFormat>().unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)), "{err:?}");
    }

    #[test]
    fn from_str_rejects_unknown_format() {
        let err = "xml".parse::<OutputFormat>().unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)), "{err:?}");
    }

    #[test]
    fn json_renders_parseable_array() {
        let s = render(&fixture(), &OutputFormat::Json).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert!(v.is_array());
        assert_eq!(v[0]["name"], "alpha");
    }

    #[test]
    fn yaml_renders_parseable_array() {
        let s = render(&fixture(), &OutputFormat::Yaml).unwrap();
        let v: serde_yaml::Value = serde_yaml::from_str(&s).unwrap();
        assert!(v.is_sequence());
    }

    #[test]
    fn name_emits_one_per_line() {
        let s = render(&fixture(), &OutputFormat::Name).unwrap();
        assert_eq!(s, "alpha\nbeta\n");
    }

    #[test]
    fn table_includes_headers_and_every_row() {
        let s = render(&fixture(), &OutputFormat::Table).unwrap();
        assert!(s.contains("ID"), "headers missing: {s}");
        assert!(s.contains("NAME"), "headers missing: {s}");
        assert!(s.contains("alpha"), "row missing: {s}");
        assert!(s.contains("beta"), "row missing: {s}");
    }

    #[test]
    fn jsonpath_filters_field_across_array() {
        let fmt = OutputFormat::JsonPath("{.[*].name}".into());
        let s = render(&fixture(), &fmt).unwrap();
        // One match per row, unquoted strings.
        assert_eq!(s, "alpha\nbeta\n");
    }

    #[test]
    fn jsonpath_rejects_malformed_expression() {
        // Outer parens are not valid jsonpath-rust syntax.
        let fmt = OutputFormat::JsonPath("garbage(((".into());
        let err = render(&fixture(), &fmt).unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)), "{err:?}");
    }

    #[test]
    fn default_for_tty_respects_explicit_flag() {
        let got = default_for_tty(Some(OutputFormat::Yaml));
        assert_eq!(got, OutputFormat::Yaml);
    }

    #[test]
    fn default_for_tty_picks_json_when_not_tty() {
        // Unit tests run with stdout redirected by the test harness,
        // so `IsTerminal` should already be false. PCY_NO_TTY=1 locks
        // the behaviour in case a future harness changes that.
        std::env::set_var("PCY_NO_TTY", "1");
        let got = default_for_tty(None);
        std::env::remove_var("PCY_NO_TTY");
        assert_eq!(got, OutputFormat::Json);
    }

    #[test]
    fn no_color_reads_env() {
        std::env::set_var("NO_COLOR", "1");
        assert!(no_color());
        std::env::set_var("NO_COLOR", "");
        assert!(!no_color());
        std::env::remove_var("NO_COLOR");
        assert!(!no_color());
    }

    #[test]
    fn render_value_kv_table_for_object() {
        #[derive(Serialize)]
        struct Me {
            user_id: String,
            workspace_id: String,
        }
        let me = Me {
            user_id: "u1".into(),
            workspace_id: "w1".into(),
        };
        let s = render_value(&me, &OutputFormat::Table).unwrap();
        assert!(s.contains("KEY"));
        assert!(s.contains("VALUE"));
        assert!(s.contains("user_id"));
        assert!(s.contains("u1"));
    }
}
