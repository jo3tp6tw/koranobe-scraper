// Declarative scraping template (v2 schema).
//
// Encodes the entire scraping flow in JSON: where to fetch, how to extract,
// how to paginate, and how one page fans out into another (e.g. index ->
// chapters). The runtime executor in `scraper::engine` consumes this schema.
//
// See `templates/kakuyomu.json` and `templates/syosetu.json` for examples.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

use crate::core::error::{Result, StorageError};

// ---------------------------------------------------------------------------
// Top-level template
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub name: String,
    #[serde(default = "default_version")]
    pub version: u32,
    pub base_url: String,

    #[serde(default)]
    pub fetch: FetchConfig,

    /// Variables computed once per template run; available as {vars.x} in
    /// templates / extractors. Each var is a `VarSpec` describing a source.
    #[serde(default)]
    pub vars: BTreeMap<String, VarSpec>,

    /// Pages keyed by name. Conventionally one page is named "index" and one
    /// is named "chapter"; the engine has no hard requirement on names but
    /// the novel adapter does (see `scraper::engine::novel`).
    pub pages: BTreeMap<String, Page>,

    /// Optional skip/gate rules (see `docs/json.md` §9.1). Selectors live here so
    /// site-specific DOM does not need hard-coding in Rust.
    #[serde(default)]
    pub gates: Option<GatesConfig>,
}

/// When matched, the downloader skips the chapter (no login flow).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GatesConfig {
    /// Chapter page: e.g. `.novel-paid-gate` (유료 회차).
    #[serde(default)]
    pub chapter: Option<GateRule>,
    /// Index `<li>` row for an episode (matched around `{item.id}` in href).
    #[serde(default)]
    pub index_item: Option<GateRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GateRule {
    #[serde(default)]
    pub selector: Option<String>,
    /// Extra substring checks (OR). Useful when the row has no stable class.
    #[serde(default)]
    pub html_contains: Vec<String>,
}

fn default_version() -> u32 {
    1
}

impl Template {
    pub fn from_json_str(s: &str) -> Result<Self> {
        Ok(serde_json::from_str(s)?)
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let p = path.as_ref();
        if !p.exists() {
            return Err(Box::new(StorageError::FileNotFound(
                p.display().to_string(),
            )));
        }
        let content = std::fs::read_to_string(p)?;
        Self::from_json_str(&content)
    }
}

// ---------------------------------------------------------------------------
// Fetch config (HTTP behaviour)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchConfig {
    #[serde(default)]
    pub user_agent: Option<String>,
    /// Delay between sequential fetches (used for batch chapter fan-out).
    #[serde(default = "default_request_delay")]
    pub request_delay_ms: u64,
    #[serde(default)]
    pub retry: RetryConfig,
    /// Reserved for future concurrent fan-out; currently sequential.
    #[serde(default = "default_concurrency")]
    pub concurrency: u32,
}

fn default_request_delay() -> u64 {
    600
}
fn default_concurrency() -> u32 {
    1
}

impl Default for FetchConfig {
    fn default() -> Self {
        Self {
            user_agent: None,
            request_delay_ms: default_request_delay(),
            retry: RetryConfig::default(),
            concurrency: default_concurrency(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    #[serde(default = "default_retry_attempts")]
    pub attempts: u32,
    #[serde(default = "default_retry_backoff")]
    pub backoff_ms: u64,
}

fn default_retry_attempts() -> u32 {
    3
}
fn default_retry_backoff() -> u64 {
    800
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            attempts: default_retry_attempts(),
            backoff_ms: default_retry_backoff(),
        }
    }
}

// ---------------------------------------------------------------------------
// Vars
// ---------------------------------------------------------------------------

/// A computed variable. Currently supports regex on the input URL, which
/// covers our actual need (deriving work_id from a kakuyomu URL).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "from", rename_all = "lowercase")]
pub enum VarSpec {
    /// Apply a regex to the input URL (or to a previously-computed value).
    Url(UrlVarSpec),
    /// A constant string.
    Const { value: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlVarSpec {
    /// Currently always "regex"; reserved for future strategies.
    #[serde(default = "default_url_var_type")]
    pub r#type: String,
    pub pattern: String,
    #[serde(default = "default_regex_group")]
    pub group: usize,
}

fn default_url_var_type() -> String {
    "regex".into()
}
fn default_regex_group() -> usize {
    1
}

// ---------------------------------------------------------------------------
// Page
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    /// URL template. Available placeholders:
    ///   - `{input}`: the URL passed in by the caller (for the entry page)
    ///   - `{vars.x}`: any computed var
    ///   - `{item.x}`: when `iterate` is set, the current item's field
    pub url: String,

    /// If set, this page is fanned out: the executor iterates over the array
    /// produced by this dotted path (e.g. `pages.index.outputs.chapters`)
    /// and runs this page once per item, exposing `{item.x}` placeholders.
    #[serde(default)]
    pub iterate: Option<String>,

    pub source: Source,

    /// Output named extractors, evaluated against the loaded source.
    #[serde(default)]
    pub outputs: BTreeMap<String, Extractor>,
}

// ---------------------------------------------------------------------------
// Source: where the data lives once we've fetched the URL
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Source {
    /// Treat the response as HTML; extractors run via CSS selectors.
    Html {
        #[serde(default)]
        paginate: Option<Paginate>,
    },
    /// Parse `<script id="__NEXT_DATA__">` JSON and use it as the JSON root
    /// for jsonpath extractors. CSS extractors still run against the original
    /// HTML body.
    NextData {
        /// If true, recursively replace `{ "__ref": "Foo:1" }` objects with
        /// the corresponding entry in `props.pageProps.__APOLLO_STATE__`.
        #[serde(default)]
        deref_refs: bool,
        /// Optional root narrowing (jsonpath) applied after deref. If unset,
        /// the root stays at the top of the parsed __NEXT_DATA__ object.
        #[serde(default)]
        root: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Pagination
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paginate {
    #[serde(default = "default_paginate_strategy")]
    pub strategy: PaginateStrategy,

    /// CSS selector for the "next" link (next_link strategy).
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default = "default_next_attr")]
    pub attr: String,
    #[serde(default = "default_resolve")]
    pub resolve: ResolveMode,

    /// Names of list-typed outputs to concatenate across pages. Other outputs
    /// are taken from the first page only.
    #[serde(default)]
    pub merge: Vec<String>,

    /// Safety cap on page count.
    #[serde(default = "default_max_pages")]
    pub max_pages: u32,
}

fn default_paginate_strategy() -> PaginateStrategy {
    PaginateStrategy::NextLink
}
fn default_next_attr() -> String {
    "href".into()
}
fn default_max_pages() -> u32 {
    100
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaginateStrategy {
    NextLink,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolveMode {
    /// Leave the value as-is.
    Raw,
    /// Resolve relative URLs against the current page URL (or `base_url`
    /// when the value is path-only).
    Absolute,
}

fn default_resolve() -> ResolveMode {
    ResolveMode::Absolute
}

// ---------------------------------------------------------------------------
// Extractors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Extractor {
    /// CSS selector against HTML.
    Css(CssSpec),
    /// JSONPath against the JSON root (only meaningful for NextData / Json sources).
    Jsonpath(JsonPathSpec),
    /// Constant value.
    Const(ConstSpec),
    /// Template string with `{x}` interpolation.
    Template(TemplateSpec),
    /// List: enumerate items (CSS containers or JSONPath array) and run the
    /// per-item sub-extractors.
    List(ListSpec),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CssSpec {
    pub selector: String,
    /// What to extract from each matched element: text (default), html, attr.
    #[serde(default = "default_css_extract")]
    pub extract: CssExtractMode,
    /// Required when `extract = "attr"`.
    #[serde(default)]
    pub attr: Option<String>,
    /// Resolve mode for the produced string (URL-ish values).
    #[serde(default)]
    pub resolve: Option<ResolveMode>,
    /// Return all matches as an array (true) instead of just the first (false).
    #[serde(default)]
    pub multiple: bool,
    /// Post-processing operations applied to the string value.
    #[serde(default)]
    pub post: Vec<PostOp>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CssExtractMode {
    Text,
    Html,
    Attr,
}

fn default_css_extract() -> CssExtractMode {
    CssExtractMode::Text
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonPathSpec {
    pub path: String,
    /// If true and path matches an array, return as JSON array; otherwise
    /// return the first match coerced to string.
    #[serde(default)]
    pub multiple: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstSpec {
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSpec {
    pub value: String,
    #[serde(default)]
    pub resolve: Option<ResolveMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSpec {
    /// CSS selector for item containers (HTML mode).
    #[serde(default)]
    pub selector: Option<String>,
    /// JSONPath for items (JSON mode).
    #[serde(default)]
    pub path: Option<String>,
    /// Per-item sub-extractors.
    pub item: BTreeMap<String, Extractor>,
}

// ---------------------------------------------------------------------------
// Post-processing operations on string values
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum PostOp {
    /// Run a CSS selector against the current value (treated as HTML),
    /// collect text from each match, and join with `join` (default: "\n").
    SelectAll {
        selector: String,
        #[serde(default = "default_join")]
        join: String,
    },
    /// String trim.
    Trim,
    /// Apply a regex to the current string; returns capture group (default 1).
    Regex {
        pattern: String,
        #[serde(default = "default_regex_group")]
        group: usize,
    },
}

fn default_join() -> String {
    "\n".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_template() {
        let json = r#"{
            "name": "demo",
            "base_url": "https://example.com",
            "pages": {
                "index": {
                    "url": "{input}",
                    "source": { "type": "html" },
                    "outputs": {
                        "title": { "type": "css", "selector": "h1" }
                    }
                }
            }
        }"#;
        let t = Template::from_json_str(json).expect("parse");
        assert_eq!(t.name, "demo");
        assert_eq!(t.version, 1);
        assert_eq!(t.fetch.request_delay_ms, 600);
        assert!(t.pages.contains_key("index"));
    }

    #[test]
    fn parses_paginate_and_list() {
        let json = r#"{
            "name": "demo",
            "base_url": "https://example.com",
            "pages": {
                "index": {
                    "url": "{input}",
                    "source": {
                        "type": "html",
                        "paginate": {
                            "selector": "a.next",
                            "merge": ["chapters"]
                        }
                    },
                    "outputs": {
                        "chapters": {
                            "type": "list",
                            "selector": ".item",
                            "item": {
                                "title": { "type": "css", "selector": "a" },
                                "url": { "type": "css", "selector": "a", "extract": "attr", "attr": "href", "resolve": "absolute" }
                            }
                        }
                    }
                }
            }
        }"#;
        let t = Template::from_json_str(json).expect("parse");
        let page = &t.pages["index"];
        match &page.source {
            Source::Html { paginate } => {
                let p = paginate.as_ref().unwrap();
                assert_eq!(p.merge, vec!["chapters".to_string()]);
            }
            _ => panic!("expected html source"),
        }
    }
}
