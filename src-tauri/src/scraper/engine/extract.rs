// Extractor execution: turn an `Extractor` spec + `PageContext` into a JSON value.

use scraper::{Html, Selector};
use serde_json::{Map, Value};

use crate::core::error::{ScrapeError, ScrapeResult};
use crate::templates::schema::{
    CssExtractMode, CssSpec, Extractor, JsonPathSpec, ListSpec, PostOp, ResolveMode,
    TemplateSpec,
};

use super::{path, render, RunScope};

/// Context passed to extractors. May carry either HTML, JSON root, or both
/// (in the case of NextData where CSS extractors still run against the
/// original HTML body).
#[derive(Debug, Clone)]
pub struct PageContext {
    pub html: Option<String>,
    pub json_root: Option<Value>,
    pub current_url: String,
    pub base_url: String,
}

impl PageContext {
    fn parse_html(&self) -> ScrapeResult<Html> {
        let h = self.html.as_deref().ok_or_else(|| {
            ScrapeError::ExtractError("CSS extractor used but no HTML in context".into())
        })?;
        Ok(Html::parse_document(h))
    }

    fn parse_html_fragment(html: &str) -> Html {
        Html::parse_fragment(html)
    }

    fn parse_selector(s: &str) -> ScrapeResult<Selector> {
        Selector::parse(s).map_err(|e| ScrapeError::SelectorError(e.to_string()))
    }
}

pub fn run_extractor(
    spec: &Extractor,
    ctx: &PageContext,
    scope: &RunScope,
) -> ScrapeResult<Value> {
    match spec {
        Extractor::Css(s) => run_css(s, ctx, scope),
        Extractor::Jsonpath(s) => run_jsonpath(s, ctx, scope),
        Extractor::Const(s) => Ok(s.value.clone()),
        Extractor::Template(s) => run_template(s, ctx, scope),
        Extractor::List(s) => run_list(s, ctx, scope),
    }
}

fn run_css(spec: &CssSpec, ctx: &PageContext, _scope: &RunScope) -> ScrapeResult<Value> {
    let doc = ctx.parse_html()?;
    let sel = PageContext::parse_selector(&spec.selector)?;
    let raws: Vec<String> = doc
        .select(&sel)
        .filter_map(|el| match spec.extract {
            CssExtractMode::Text => Some(normalize_ws(&el.text().collect::<String>())),
            CssExtractMode::Html => Some(el.inner_html()),
            CssExtractMode::Attr => spec
                .attr
                .as_deref()
                .and_then(|a| el.value().attr(a).map(|s| s.to_string())),
        })
        .collect();

    let processed: ScrapeResult<Vec<String>> = raws
        .into_iter()
        .map(|s| {
            let s = apply_post(&spec.post, s)?;
            let s = match spec.resolve {
                Some(mode) => render::resolve_url(&s, &ctx.current_url, mode)?,
                None => s,
            };
            Ok(s)
        })
        .collect();
    let processed = processed?;

    if spec.multiple {
        Ok(Value::Array(
            processed.into_iter().map(Value::String).collect(),
        ))
    } else {
        Ok(processed
            .into_iter()
            .next()
            .map(Value::String)
            .unwrap_or(Value::String(String::new())))
    }
}

fn run_jsonpath(
    spec: &JsonPathSpec,
    ctx: &PageContext,
    _scope: &RunScope,
) -> ScrapeResult<Value> {
    let root = ctx.json_root.as_ref().ok_or_else(|| {
        ScrapeError::ExtractError("jsonpath extractor used but no JSON root in context".into())
    })?;
    let matches = path::select(root, &spec.path)?;
    if spec.multiple {
        Ok(Value::Array(matches.into_iter().cloned().collect()))
    } else {
        Ok(matches
            .into_iter()
            .next()
            .cloned()
            .unwrap_or(Value::Null))
    }
}

fn run_template(spec: &TemplateSpec, ctx: &PageContext, scope: &RunScope) -> ScrapeResult<Value> {
    let s = render::render_string(&spec.value, &ctx.base_url, scope, None)?;
    let s = match spec.resolve {
        Some(mode) => render::resolve_url(&s, &ctx.current_url, mode)?,
        None => s,
    };
    Ok(Value::String(s))
}

fn run_list(spec: &ListSpec, ctx: &PageContext, scope: &RunScope) -> ScrapeResult<Value> {
    if let Some(selector) = &spec.selector {
        // HTML mode: each matched element becomes a sub-context's html fragment.
        let doc = ctx.parse_html()?;
        let sel = PageContext::parse_selector(selector)?;
        let mut out: Vec<Value> = Vec::new();
        for el in doc.select(&sel) {
            let frag_html = el.html();
            let sub_ctx = PageContext {
                html: Some(frag_html),
                json_root: ctx.json_root.clone(),
                current_url: ctx.current_url.clone(),
                base_url: ctx.base_url.clone(),
            };
            // Sub-extractors are evaluated and the resulting object becomes
            // the iteration item, so later sub-extractors (and templates that
            // reference `{item.x}`) can use values produced by earlier ones.
            let mut obj = Map::new();
            let mut sub_scope = scope.clone();
            for (k, sub_spec) in &spec.item {
                sub_scope.item = Some(Value::Object(obj.clone()));
                let v = run_extractor(sub_spec, &sub_ctx, &sub_scope)?;
                obj.insert(k.clone(), v);
            }
            out.push(Value::Object(obj));
        }
        Ok(Value::Array(out))
    } else if let Some(json_path) = &spec.path {
        let root = ctx.json_root.as_ref().ok_or_else(|| {
            ScrapeError::ExtractError("list path used but no JSON root in context".into())
        })?;
        let matches = path::select(root, json_path)?;
        let mut out: Vec<Value> = Vec::new();
        for v in matches {
            let sub_ctx = PageContext {
                html: ctx.html.clone(),
                json_root: Some(v.clone()),
                current_url: ctx.current_url.clone(),
                base_url: ctx.base_url.clone(),
            };
            // Seed `{item.x}` with the raw JSON item so per-item templates
            // (e.g. URL builders) can read its fields. Sub-extractor outputs
            // are then merged in as they're computed, so later sub-extractors
            // can also reference sibling outputs.
            let mut sub_scope = scope.clone();
            let mut working_item = v.clone();
            let mut obj = Map::new();
            for (k, sub_spec) in &spec.item {
                sub_scope.item = Some(working_item.clone());
                let val = run_extractor(sub_spec, &sub_ctx, &sub_scope)?;
                if let Value::Object(map) = &mut working_item {
                    map.insert(k.clone(), val.clone());
                }
                obj.insert(k.clone(), val);
            }
            out.push(Value::Object(obj));
        }
        Ok(Value::Array(out))
    } else {
        Err(ScrapeError::ExtractError(
            "list extractor requires `selector` or `path`".into(),
        ))
    }
}

fn apply_post(post: &[PostOp], mut s: String) -> ScrapeResult<String> {
    for op in post {
        s = match op {
            PostOp::SelectAll { selector, join } => {
                let frag = PageContext::parse_html_fragment(&s);
                let sel = PageContext::parse_selector(selector)?;
                let parts: Vec<String> = frag
                    .select(&sel)
                    .map(|el| normalize_ws(&el.text().collect::<String>()))
                    .filter(|t| !t.is_empty())
                    .collect();
                parts.join(join)
            }
            PostOp::Trim => s.trim().to_string(),
            PostOp::Regex { pattern, group } => {
                let re = regex::Regex::new(pattern).map_err(|e| {
                    ScrapeError::ExtractError(format!("post regex compile failed: {}", e))
                })?;
                let caps = re.captures(&s).ok_or_else(|| {
                    ScrapeError::ExtractError(format!(
                        "post regex `{}` did not match `{}`",
                        pattern, s
                    ))
                })?;
                caps.get(*group)
                    .map(|m| m.as_str().to_string())
                    .ok_or_else(|| {
                        ScrapeError::ExtractError(format!(
                            "post regex group {} missing for `{}`",
                            group, pattern
                        ))
                    })?
            }
        };
    }
    Ok(s)
}

fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx_with_html(html: &str) -> PageContext {
        PageContext {
            html: Some(html.to_string()),
            json_root: None,
            current_url: "https://example.com/page/".into(),
            base_url: "https://example.com".into(),
        }
    }

    #[test]
    fn css_text_extraction() {
        let ctx = ctx_with_html("<div><h1>Hello</h1></div>");
        let spec = Extractor::Css(CssSpec {
            selector: "h1".into(),
            extract: CssExtractMode::Text,
            attr: None,
            resolve: None,
            multiple: false,
            post: vec![],
        });
        let scope = RunScope::default();
        let v = run_extractor(&spec, &ctx, &scope).unwrap();
        assert_eq!(v, json!("Hello"));
    }

    #[test]
    fn css_attr_with_resolve() {
        let ctx = ctx_with_html(r#"<a href="/foo">x</a>"#);
        let spec = Extractor::Css(CssSpec {
            selector: "a".into(),
            extract: CssExtractMode::Attr,
            attr: Some("href".into()),
            resolve: Some(ResolveMode::Absolute),
            multiple: false,
            post: vec![],
        });
        let scope = RunScope::default();
        let v = run_extractor(&spec, &ctx, &scope).unwrap();
        assert_eq!(v, json!("https://example.com/foo"));
    }

    #[test]
    fn list_with_html_container() {
        let html = r#"
            <ul>
              <li><a href="/a">A</a></li>
              <li><a href="/b">B</a></li>
            </ul>"#;
        let ctx = ctx_with_html(html);
        let mut item = std::collections::BTreeMap::new();
        item.insert(
            "title".into(),
            Extractor::Css(CssSpec {
                selector: "a".into(),
                extract: CssExtractMode::Text,
                attr: None,
                resolve: None,
                multiple: false,
                post: vec![],
            }),
        );
        item.insert(
            "url".into(),
            Extractor::Css(CssSpec {
                selector: "a".into(),
                extract: CssExtractMode::Attr,
                attr: Some("href".into()),
                resolve: Some(ResolveMode::Absolute),
                multiple: false,
                post: vec![],
            }),
        );
        let spec = Extractor::List(ListSpec {
            selector: Some("li".into()),
            path: None,
            item,
        });
        let scope = RunScope::default();
        let v = run_extractor(&spec, &ctx, &scope).unwrap();
        assert_eq!(
            v,
            json!([
                { "title": "A", "url": "https://example.com/a" },
                { "title": "B", "url": "https://example.com/b" }
            ])
        );
    }

    #[test]
    fn post_select_all_joins_paragraphs() {
        let html =
            r#"<div class="body"><p>line one</p><p>line two</p></div>"#;
        let ctx = ctx_with_html(html);
        let spec = Extractor::Css(CssSpec {
            selector: ".body".into(),
            extract: CssExtractMode::Html,
            attr: None,
            resolve: None,
            multiple: false,
            post: vec![PostOp::SelectAll {
                selector: "p".into(),
                join: "\n".into(),
            }],
        });
        let scope = RunScope::default();
        let v = run_extractor(&spec, &ctx, &scope).unwrap();
        assert_eq!(v, json!("line one\nline two"));
    }
}
