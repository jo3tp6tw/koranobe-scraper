// Template-driven gate rules (`gates` in JSON).

use scraper::{Html, Selector};

use crate::templates::schema::{Extractor, GateRule, Template};

/// True if any `html_contains` needle appears in `html`.
pub fn html_contains_match(html: &str, rule: &GateRule) -> bool {
    rule.html_contains
        .iter()
        .any(|n| !n.is_empty() && html.contains(n))
}

/// True if `selector` matches at least one node in `html` (fragment or document).
pub fn selector_match(html: &str, rule: &GateRule) -> bool {
    let Some(sel_str) = rule.selector.as_deref() else {
        return false;
    };
    let Ok(sel) = Selector::parse(sel_str) else {
        return false;
    };
    let doc = Html::parse_fragment(html);
    doc.select(&sel).next().is_some()
}

pub fn gate_matches(html: &str, rule: &GateRule) -> bool {
    selector_match(html, rule) || html_contains_match(html, rule)
}

/// True if `fragment` (usually one index `<li>` inner HTML) links to `episode_id`.
fn li_contains_episode_id(fragment: &str, episode_id: &str) -> bool {
    if episode_id.is_empty() {
        return false;
    }
    let needle = format!("/{}", episode_id);
    fragment.match_indices(&needle).any(|(pos, _)| {
        let rest = &fragment[pos + needle.len()..];
        if rest.is_empty() {
            return true;
        }
        match rest.as_bytes()[0] {
            b'"' | b'\'' | b'>' | b' ' | b'#' | b'/' | b'&' | b'?' => true,
            c if c.is_ascii_digit() => false,
            _ => true,
        }
    })
}

/// `pages.index.outputs.chapters` list selector from the template (site-specific).
pub fn index_chapters_row_selector(template: &Template) -> Option<&str> {
    match template
        .pages
        .get("index")?
        .outputs
        .get("chapters")
    {
        Some(Extractor::List(spec)) => spec.selector.as_deref(),
        _ => None,
    }
}

/// Index row: find the list row for `episode_id`, then apply `gates.index_item`.
pub fn index_episode_gated(
    index_html: &str,
    episode_id: &str,
    rule: &GateRule,
    row_selector: Option<&str>,
) -> bool {
    if episode_id.is_empty() {
        return false;
    }

    if let Some(sel_str) = row_selector {
        if let Ok(li_sel) = Selector::parse(sel_str) {
            let doc = Html::parse_document(index_html);
            for li in doc.select(&li_sel) {
                let frag = li.html();
                if li_contains_episode_id(&frag, episode_id) && gate_matches(&frag, rule) {
                    return true;
                }
            }
        }
    }

    // Fallback when the document is a fragment or non-standard markup.
    for li_chunk in index_html.split("<li").skip(1) {
        if li_chunk_matches(li_chunk, episode_id, rule) {
            return true;
        }
    }
    if index_html.contains("<LI") {
        for li_chunk in index_html.split("<LI").skip(1) {
            if li_chunk_matches(li_chunk, episode_id, rule) {
                return true;
            }
        }
    }
    false
}

fn li_chunk_matches(li_chunk: &str, episode_id: &str, rule: &GateRule) -> bool {
    li_contains_episode_id(li_chunk, episode_id) && gate_matches(li_chunk, rule)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chapter_rule() -> GateRule {
        GateRule {
            selector: Some("article.novel-viewer .novel-paid-gate".into()),
            html_contains: vec!["novel-paid-gate".into(), "유료 회차".into()],
        }
    }

    fn index_rule() -> GateRule {
        GateRule {
            selector: Some(".ne-pt, .novel-ep-pt, .ep-row-v2-pt".into()),
            html_contains: vec![
                "<!-- -->P".into(),
                ">P</span>".into(),
                "novel-ep-pt".into(),
                "ne-pt".into(),
            ],
        }
    }

    #[test]
    fn chapter_gate_by_selector() {
        let html = r#"<article class="novel-viewer"><div class="novel-paid-gate"><h3>유료 회차</h3></div></article>"#;
        assert!(gate_matches(html, &chapter_rule()));
    }

    #[test]
    fn index_gate_by_marker_in_li() {
        let html = r#"<ul><li class="novel-ep-row"><a href="/novel/1/5470216">x</a><span>P</span></li><li class="novel-ep-row"><a href="/novel/1/5470214">y</a></li></ul>"#;
        let rule = GateRule {
            selector: None,
            html_contains: vec![">P</span>".into()],
        };
        let row_sel = "li";
        assert!(index_episode_gated(html, "5470216", &rule, Some(row_sel)));
        assert!(!index_episode_gated(html, "5470214", &rule, Some(row_sel)));
    }

    #[test]
    fn index_gate_full_url_href() {
        let html = r#"<ul class="novel-eps"><li class="novel-ep-row"><a class="novel-ep-link" href="https://sbxh1.com/novel/58182/5470216">11</a><span>2</span><span class="ne-pt">P</span></li><li class="novel-ep-row novel-ep--read"><a href="https://sbxh1.com/novel/58182/5470214">10</a></li></ul>"#;
        let row_sel = "ul.novel-eps > li.novel-ep-row, .ep-list-v2 li";
        assert!(index_episode_gated(html, "5470216", &index_rule(), Some(row_sel)));
        assert!(!index_episode_gated(html, "5470214", &index_rule(), Some(row_sel)));
    }

    #[test]
    fn index_gate_vue_comment_p_marker() {
        let html = r#"<ul class="novel-eps"><li class="novel-ep-row"><a href="/novel/58182/5470216">11</a><span class="ne-pt">2<!-- -->P</span></li><li class="novel-ep-row"><a href="/novel/58182/5470214">10</a></li></ul>"#;
        let row_sel = "ul.novel-eps > li.novel-ep-row";
        assert!(index_episode_gated(html, "5470216", &index_rule(), Some(row_sel)));
        assert!(!index_episode_gated(html, "5470214", &index_rule(), Some(row_sel)));
    }

    #[test]
    fn index_gate_href_without_trailing_slash() {
        let html = r#"<ul><li class="novel-ep-row"><a href="/novel/58182/5470216">x</a><span class="ne-pt">P</span></li></ul>"#;
        assert!(index_episode_gated(
            html,
            "5470216",
            &index_rule(),
            Some("li.novel-ep-row")
        ));
    }
}
