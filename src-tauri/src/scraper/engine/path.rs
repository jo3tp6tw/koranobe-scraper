// Minimal JSONPath-ish resolver.
//
// Supports a strict subset that covers our templates:
//   - `$`             root
//   - `.field`        field access
//   - `['Foo:123']`   bracket field access (allows colons / unusual chars)
//   - `[*]`           wildcard over an array (yields multiple values)
//   - chained, e.g. `$.props.pageProps.apolloState['Work:1'].tableOfContentsV2[*].episodeUnions[*]`
//
// No filters, no recursive descent, no slicing. Returns a Vec<Value> because
// wildcards can fan out.

use serde_json::Value;

use crate::core::error::{ScrapeError, ScrapeResult};

#[derive(Debug, Clone)]
enum Step {
    Field(String),
    Wildcard,
}

fn tokenize(path: &str) -> ScrapeResult<Vec<Step>> {
    let p = path.trim();
    let p = p.strip_prefix('$').unwrap_or(p);
    let bytes: Vec<char> = p.chars().collect();
    let mut steps = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            '.' => {
                // .field
                i += 1;
                let start = i;
                while i < bytes.len() && bytes[i] != '.' && bytes[i] != '[' {
                    i += 1;
                }
                if start == i {
                    return Err(ScrapeError::ExtractError(format!(
                        "jsonpath: empty field after '.': {}",
                        path
                    )));
                }
                steps.push(Step::Field(bytes[start..i].iter().collect()));
            }
            '[' => {
                i += 1;
                if i < bytes.len() && bytes[i] == '*' {
                    i += 1;
                    if i >= bytes.len() || bytes[i] != ']' {
                        return Err(ScrapeError::ExtractError(format!(
                            "jsonpath: expected ']' after [*]: {}",
                            path
                        )));
                    }
                    i += 1;
                    steps.push(Step::Wildcard);
                } else if i < bytes.len() && (bytes[i] == '\'' || bytes[i] == '"') {
                    let quote = bytes[i];
                    i += 1;
                    let start = i;
                    while i < bytes.len() && bytes[i] != quote {
                        i += 1;
                    }
                    if i >= bytes.len() {
                        return Err(ScrapeError::ExtractError(format!(
                            "jsonpath: unterminated string in []: {}",
                            path
                        )));
                    }
                    let key: String = bytes[start..i].iter().collect();
                    i += 1; // closing quote
                    if i >= bytes.len() || bytes[i] != ']' {
                        return Err(ScrapeError::ExtractError(format!(
                            "jsonpath: expected ']' after string: {}",
                            path
                        )));
                    }
                    i += 1;
                    steps.push(Step::Field(key));
                } else {
                    return Err(ScrapeError::ExtractError(format!(
                        "jsonpath: only [*] and ['key'] supported inside []: {}",
                        path
                    )));
                }
            }
            c if c.is_whitespace() => i += 1,
            other => {
                return Err(ScrapeError::ExtractError(format!(
                    "jsonpath: unexpected char '{}' at {}: {}",
                    other, i, path
                )));
            }
        }
    }
    Ok(steps)
}

/// Resolve `path` against `root`, returning all matched values.
pub fn select<'a>(root: &'a Value, path: &str) -> ScrapeResult<Vec<&'a Value>> {
    let steps = tokenize(path)?;
    let mut current: Vec<&Value> = vec![root];
    for step in steps {
        let mut next: Vec<&Value> = Vec::new();
        match step {
            Step::Field(name) => {
                for v in current {
                    if let Some(child) = v.get(&name) {
                        next.push(child);
                    }
                }
            }
            Step::Wildcard => {
                for v in current {
                    if let Some(arr) = v.as_array() {
                        for item in arr {
                            next.push(item);
                        }
                    } else if let Some(obj) = v.as_object() {
                        for (_, item) in obj {
                            next.push(item);
                        }
                    }
                }
            }
        }
        current = next;
    }
    Ok(current)
}

/// Convenience: select first match.
pub fn select_first<'a>(root: &'a Value, path: &str) -> ScrapeResult<Option<&'a Value>> {
    Ok(select(root, path)?.into_iter().next())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn field_and_bracket_access() {
        let root = json!({
            "props": {
                "pageProps": {
                    "apolloState": {
                        "Work:1": { "title": "T" }
                    }
                }
            }
        });
        let r = select(&root, "$.props.pageProps.apolloState['Work:1'].title").unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0], &json!("T"));
    }

    #[test]
    fn wildcard_fans_out() {
        let root = json!({
            "items": [
                { "id": 1 },
                { "id": 2 },
                { "id": 3 }
            ]
        });
        let r = select(&root, "$.items[*].id").unwrap();
        let ids: Vec<i64> = r.into_iter().map(|v| v.as_i64().unwrap()).collect();
        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[test]
    fn nested_wildcards() {
        let root = json!({
            "toc": [
                { "eps": [ {"id": "a"}, {"id": "b"} ] },
                { "eps": [ {"id": "c"} ] }
            ]
        });
        let r = select(&root, "$.toc[*].eps[*].id").unwrap();
        let ids: Vec<String> = r
            .into_iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }
}
