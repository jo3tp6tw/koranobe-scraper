// String template + URL resolution helpers.

use serde_json::Value;
use url::Url;

use crate::core::error::{ScrapeError, ScrapeResult};
use crate::templates::schema::ResolveMode;

use super::RunScope;

/// Render a template string with `{placeholder}` interpolation.
///
/// Supported placeholders:
///   - `{input}`           the original input URL
///   - `{base_url}`        template.base_url
///   - `{vars.x}`          a computed var (dotted access supported)
///   - `{item.x.y}`        the current iteration item (dotted access)
///   - `{outputs.page.x}`  not currently supported (reserved)
pub fn render_string(
    template: &str,
    base_url: &str,
    scope: &RunScope,
    extra: Option<&Value>,
) -> ScrapeResult<String> {
    let mut out = String::with_capacity(template.len());
    let bytes: Vec<char> = template.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == '{' {
            // find matching '}'
            let start = i + 1;
            let end = bytes[start..]
                .iter()
                .position(|&c| c == '}')
                .map(|p| start + p)
                .ok_or_else(|| {
                    ScrapeError::ExtractError(format!(
                        "template: unterminated `{{` in `{}`",
                        template
                    ))
                })?;
            let key: String = bytes[start..end].iter().collect();
            let value = resolve_placeholder(&key, base_url, scope, extra)?;
            out.push_str(&value);
            i = end + 1;
        } else {
            out.push(c);
            i += 1;
        }
    }
    Ok(out)
}

fn resolve_placeholder(
    key: &str,
    base_url: &str,
    scope: &RunScope,
    extra: Option<&Value>,
) -> ScrapeResult<String> {
    let key = key.trim();
    if key == "input" {
        return Ok(scope.input_url.clone());
    }
    if key == "base_url" {
        return Ok(base_url.to_string());
    }
    if let Some(rest) = key.strip_prefix("vars.") {
        return dotted_lookup(&scope.vars, rest, key);
    }
    if let Some(rest) = key.strip_prefix("item.") {
        let item = scope.item.as_ref().ok_or_else(|| {
            ScrapeError::ExtractError(format!(
                "template: `{{{}}}` used outside iterate scope",
                key
            ))
        })?;
        return dotted_lookup(item, rest, key);
    }
    if key == "item" {
        if let Some(it) = &scope.item {
            return Ok(value_to_string(it));
        }
    }
    if let Some(rest) = key.strip_prefix("extra.") {
        let v = extra.ok_or_else(|| {
            ScrapeError::ExtractError(format!("template: extra not provided for `{{{}}}`", key))
        })?;
        return dotted_lookup(v, rest, key);
    }
    Err(ScrapeError::ExtractError(format!(
        "template: unknown placeholder `{{{}}}`",
        key
    )))
}

fn dotted_lookup(root: &Value, path: &str, full: &str) -> ScrapeResult<String> {
    let mut cur = root;
    for part in path.split('.') {
        cur = cur.get(part).ok_or_else(|| {
            ScrapeError::ExtractError(format!(
                "template: missing field `{}` while resolving `{{{}}}`",
                part, full
            ))
        })?;
    }
    Ok(value_to_string(cur))
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Resolve a (possibly relative) URL according to `mode`, using `base` as the
/// base for relative resolution. `base` is typically the current page URL so
/// that path-only / query-only relatives work.
pub fn resolve_url(value: &str, base: &str, mode: ResolveMode) -> ScrapeResult<String> {
    match mode {
        ResolveMode::Raw => Ok(value.to_string()),
        ResolveMode::Absolute => {
            if value.starts_with("http://") || value.starts_with("https://") {
                return Ok(value.to_string());
            }
            let base_url = Url::parse(base).map_err(|e| {
                ScrapeError::InvalidUrl(format!("base `{}`: {}", base, e))
            })?;
            let joined = base_url.join(value).map_err(|e| {
                ScrapeError::InvalidUrl(format!("join `{}` onto `{}`: {}", value, base, e))
            })?;
            Ok(joined.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn renders_input_and_vars() {
        let mut scope = RunScope::new("https://x.com/works/123");
        scope.vars = json!({ "work_id": "123" });
        let r = render_string(
            "{base_url}/works/{vars.work_id}/episodes/{item.id}",
            "https://x.com",
            &RunScope {
                vars: json!({ "work_id": "123" }),
                item: Some(json!({ "id": "ep1" })),
                ..scope
            },
            None,
        )
        .unwrap();
        assert_eq!(r, "https://x.com/works/123/episodes/ep1");
    }

    #[test]
    fn resolves_relative_url_with_query_only() {
        let r = resolve_url(
            "?p=2",
            "https://ncode.syosetu.com/n2468mc/",
            ResolveMode::Absolute,
        )
        .unwrap();
        assert_eq!(r, "https://ncode.syosetu.com/n2468mc/?p=2");
    }

    #[test]
    fn resolves_absolute_url_passthrough() {
        let r = resolve_url(
            "https://other.com/x",
            "https://ncode.syosetu.com/",
            ResolveMode::Absolute,
        )
        .unwrap();
        assert_eq!(r, "https://other.com/x");
    }

    #[test]
    fn resolves_path_relative() {
        let r = resolve_url(
            "/n2468mc/2/",
            "https://ncode.syosetu.com/n2468mc/",
            ResolveMode::Absolute,
        )
        .unwrap();
        assert_eq!(r, "https://ncode.syosetu.com/n2468mc/2/");
    }
}
