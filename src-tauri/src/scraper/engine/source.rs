// Parse pre-fetched HTML with template extractors (WebView path only).

use serde_json::{Map, Value};

use crate::core::error::ScrapeResult;
use crate::templates::schema::Page;

use super::extract::{run_extractor, PageContext};
use super::Engine;

fn build_html_ctx(engine: &Engine, html: &str, url: &str) -> PageContext {
    PageContext {
        html: Some(html.to_string()),
        json_root: None,
        current_url: url.to_string(),
        base_url: engine.template.base_url.clone(),
    }
}

fn run_outputs(page: &Page, ctx: &PageContext, scope: &super::RunScope) -> ScrapeResult<Value> {
    let mut out = Map::new();
    for (k, spec) in &page.outputs {
        let v = run_extractor(spec, ctx, scope)?;
        out.insert(k.clone(), v);
    }
    Ok(Value::Object(out))
}

/// Run all outputs of `page` against a pre-fetched HTML body.
pub fn run_outputs_from_html(
    engine: &Engine,
    page: &Page,
    html: &str,
    url: &str,
    scope: &super::RunScope,
) -> ScrapeResult<Value> {
    let ctx = build_html_ctx(engine, html, url);
    run_outputs(page, &ctx, scope)
}

#[cfg(test)]
pub(crate) fn run_outputs_for_html_sync(
    engine: &Engine,
    page: &Page,
    html: &str,
    url: &str,
    scope: &super::RunScope,
) -> ScrapeResult<Value> {
    run_outputs_from_html(engine, page, html, url, scope)
}
