// High-level novel adapter on top of the generic engine.

use serde_json::Value;

use crate::core::error::{ScrapeError, ScrapeResult};
use crate::templates::{Chapter, ChapterContent, NovelInfo};

use super::source;
use super::{Engine, RunScope};

const INDEX_PAGE: &str = "index";
const CHAPTER_PAGE: &str = "chapter";

fn index_page(engine: &Engine) -> ScrapeResult<&crate::templates::schema::Page> {
    engine
        .template
        .pages
        .get(INDEX_PAGE)
        .ok_or_else(|| ScrapeError::ExtractError(format!("template.pages missing `{}`", INDEX_PAGE)))
}

fn chapter_page(engine: &Engine) -> ScrapeResult<&crate::templates::schema::Page> {
    engine
        .template
        .pages
        .get(CHAPTER_PAGE)
        .ok_or_else(|| {
            ScrapeError::ExtractError(format!("template.pages missing `{}`", CHAPTER_PAGE))
        })
}

/// Parse index outputs from HTML already fetched via WebView.
pub fn extract_novel_info_from_html(
    engine: &Engine,
    input_url: &str,
    html: &str,
) -> ScrapeResult<NovelInfo> {
    let mut scope = RunScope::new(input_url);
    scope.vars = engine.compute_vars(input_url)?;
    let page = index_page(engine)?;
    let outputs = source::run_outputs_from_html(engine, page, html, input_url, &scope)?;
    map_novel_info(&outputs)
}

/// Parse chapter outputs from HTML already fetched via WebView.
pub fn extract_chapter_from_html(
    engine: &Engine,
    input_url: &str,
    chapter: &Chapter,
    html: &str,
) -> ScrapeResult<ChapterContent> {
    let mut scope = RunScope::new(input_url);
    scope.vars = engine.compute_vars(input_url)?;
    scope.item = Some(serde_json::to_value(chapter)?);
    let page = chapter_page(engine)?;
    let outputs = source::run_outputs_from_html(engine, page, html, &chapter.url, &scope)?;
    map_chapter_content(&outputs)
}

#[cfg(test)]
pub(crate) fn map_novel_info_for_test(outputs: &Value) -> ScrapeResult<NovelInfo> {
    map_novel_info(outputs)
}

fn map_novel_info(outputs: &Value) -> ScrapeResult<NovelInfo> {
    let title = string_field(outputs, "title")?;
    let author = string_field(outputs, "author").unwrap_or_default();
    let introduction = string_field(outputs, "introduction").unwrap_or_default();
    let tags = string_array_field(outputs, "tags").unwrap_or_default();

    let chapters_val = outputs.get("chapters").ok_or_else(|| {
        ScrapeError::ExtractError("index outputs missing `chapters`".into())
    })?;
    let chapters_arr = chapters_val.as_array().ok_or_else(|| {
        ScrapeError::ExtractError("index outputs.chapters must be array".into())
    })?;

    let mut chapters: Vec<Chapter> = Vec::with_capacity(chapters_arr.len());
    for (i, item) in chapters_arr.iter().enumerate() {
        let title = item
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let url = item
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let id = item
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| url.clone());
        if url.is_empty() {
            return Err(ScrapeError::ExtractError(format!(
                "chapter #{} is missing `url`",
                i + 1
            )));
        }
        let requires_login = item
            .get("paid_gate")
            .and_then(|v| v.as_str())
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        let index = item
            .get("num")
            .and_then(|v| v.as_str())
            .and_then(parse_episode_num)
            .unwrap_or(i + 1);
        chapters.push(Chapter {
            id,
            title,
            url,
            index,
            requires_login,
        });
    }

    chapters.sort_by_key(|c| c.index);

    Ok(NovelInfo {
        title,
        author,
        introduction,
        tags,
        chapters,
    })
}

fn map_chapter_content(outputs: &Value) -> ScrapeResult<ChapterContent> {
    let title = string_field(outputs, "title").unwrap_or_default();
    let content = string_field(outputs, "content")?;
    Ok(ChapterContent { title, content })
}

/// Parse episode number from template `num` field (e.g. `"1화"`, `"12화"`, `"001"`).
fn parse_episode_num(s: &str) -> Option<usize> {
    let digits: String = s
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

fn string_field(v: &Value, key: &str) -> ScrapeResult<String> {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            ScrapeError::ExtractError(format!("outputs missing string field `{}`", key))
        })
}

fn string_array_field(v: &Value, key: &str) -> ScrapeResult<Vec<String>> {
    let arr = v
        .get(key)
        .and_then(|x| x.as_array())
        .ok_or_else(|| {
            ScrapeError::ExtractError(format!("outputs missing array field `{}`", key))
        })?;
    Ok(arr
        .iter()
        .filter_map(|x| x.as_str().map(|s| s.to_string()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_episode_num_extracts_digits() {
        assert_eq!(parse_episode_num("1화"), Some(1));
        assert_eq!(parse_episode_num("12화"), Some(12));
        assert_eq!(parse_episode_num("001"), Some(1));
        assert_eq!(parse_episode_num("외전"), None);
        assert_eq!(parse_episode_num(""), None);
    }
}
