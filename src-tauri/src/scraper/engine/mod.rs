// Schema-driven HTML parsing engine (WebView-fetched HTML only).

pub mod extract;
pub mod gate;
pub mod novel;
pub mod path;
pub mod render;
pub mod source;

use serde_json::Value;

use crate::core::error::{ScrapeError, ScrapeResult};
use crate::templates::schema::{Template, VarSpec};

pub struct Engine {
    pub template: Template,
}

/// Per-run scope: input URL + computed vars + fan-out item.
#[derive(Debug, Clone, Default)]
pub struct RunScope {
    pub input_url: String,
    pub vars: Value,
    pub item: Option<Value>,
}

impl RunScope {
    pub fn new(input_url: impl Into<String>) -> Self {
        Self {
            input_url: input_url.into(),
            vars: Value::Object(serde_json::Map::new()),
            item: None,
        }
    }

    pub fn with_item(mut self, item: Value) -> Self {
        self.item = Some(item);
        self
    }
}

impl Engine {
    pub fn new(template: Template) -> ScrapeResult<Self> {
        Ok(Self { template })
    }

    /// Compute `template.vars` against the input URL.
    pub fn compute_vars(&self, input_url: &str) -> ScrapeResult<Value> {
        let mut out = serde_json::Map::new();
        for (k, spec) in &self.template.vars {
            let v = match spec {
                VarSpec::Const { value } => Value::String(value.clone()),
                VarSpec::Url(spec) => {
                    let re = regex::Regex::new(&spec.pattern).map_err(|e| {
                        ScrapeError::ExtractError(format!(
                            "var `{}`: regex `{}` compile failed: {}",
                            k, spec.pattern, e
                        ))
                    })?;
                    let captured = re
                        .captures(input_url)
                        .and_then(|c| c.get(spec.group))
                        .map(|m| m.as_str().to_string())
                        .ok_or_else(|| {
                            ScrapeError::ExtractError(format!(
                                "var `{}`: regex `{}` did not match input URL",
                                k, spec.pattern
                            ))
                        })?;
                    Value::String(captured)
                }
            };
            out.insert(k.clone(), v);
        }
        Ok(Value::Object(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templates::schema::{UrlVarSpec, VarSpec};

    fn make_template_with_var() -> Template {
        let mut t: Template = serde_json::from_str(
            r#"{
                "name": "demo",
                "base_url": "https://example.com",
                "pages": {}
            }"#,
        )
        .unwrap();
        t.vars.insert(
            "work_id".into(),
            VarSpec::Url(UrlVarSpec {
                r#type: "regex".into(),
                pattern: r"/novel/(\d+)".into(),
                group: 1,
            }),
        );
        t
    }

    #[test]
    fn compute_vars_runs_regex_against_input_url() {
        let engine = Engine::new(make_template_with_var()).unwrap();
        let v = engine
            .compute_vars("https://sbxh1.com/novel/57467")
            .unwrap();
        assert_eq!(v["work_id"], "57467");
    }
}

#[cfg(test)]
mod fixture_tests {
    use super::*;
    use crate::templates::built_in;

    const NEWTOKI_INDEX: &str = include_str!("../../../../docs/fixtures/newtoki-index");

    #[test]
    fn newtoki_index_extracts_metadata_and_chapter_urls() {
        let template = built_in::get("newtoki").expect("newtoki").expect("parse");
        let engine = Engine::new(template).unwrap();
        let url = "https://sbxh1.com/novel/57467";
        let mut scope = RunScope::new(url);
        scope.vars = engine.compute_vars(url).unwrap();

        let page = engine.template.pages.get("index").unwrap();
        let outputs =
            source::run_outputs_for_html_sync(&engine, page, NEWTOKI_INDEX, url, &scope).unwrap();
        let info = novel::map_novel_info_for_test(&outputs).unwrap();
        assert_eq!(info.title, "아이템 박스? 냉장고로 씁니다");
        assert_eq!(info.chapters.len(), 12);
        assert_eq!(info.chapters.first().unwrap().index, 1);
        assert_eq!(info.chapters.last().unwrap().index, 12);
        for w in info.chapters.windows(2) {
            assert!(w[0].index <= w[1].index);
        }
    }
}
