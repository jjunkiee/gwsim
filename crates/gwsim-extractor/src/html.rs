//! Small helpers every parser shares.
//!
//! The parsers read raw pages from the cache and hand back numbers, flags and
//! titles. Prose is read only where a flag or a hash has to be derived from
//! it, and is never returned (EXT-7).

use scraper::{ElementRef, Html, Selector};

/// A CSS selector known to be valid at compile time.
///
/// Every selector in this crate is a literal, so a parse failure is a
/// programming error, found by the first test that runs the parser.
pub fn selector(css: &str) -> Selector {
    Selector::parse(css).unwrap_or_else(|error| panic!("invalid selector {css:?}: {error:?}"))
}

/// An element's text with whitespace collapsed and trimmed.
///
/// Non-breaking spaces count as whitespace; the wiki uses them inside labels
/// such as `Sacrifice %` (T2.1 §3).
pub fn text_of(element: ElementRef<'_>) -> String {
    collapse(&element.text().collect::<String>())
}

/// Collapses runs of whitespace (including non-breaking spaces) to one space.
pub fn collapse(text: &str) -> String {
    text.split(|c: char| c.is_whitespace() || c == '\u{a0}')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The page title from the `h1#firstHeading`.
pub fn page_heading(body: &str) -> Option<String> {
    let document = Html::parse_document(body);
    document
        .select(&selector("h1#firstHeading"))
        .next()
        .map(text_of)
        .filter(|title| !title.is_empty())
}

/// The title a MediaWiki redirect page was reached from, if it was.
pub fn redirected_from(body: &str) -> Option<String> {
    if !body.contains("mw-redirectedfrom") {
        return None;
    }
    let document = Html::parse_document(body);
    document
        .select(&selector(".mw-redirectedfrom a"))
        .next()
        .and_then(|link| link.value().attr("title").map(str::to_owned))
}

/// The page's visible categories, from `#catlinks`.
pub fn categories(document: &Html) -> Vec<String> {
    document
        .select(&selector("#mw-normal-catlinks ul li a"))
        .map(text_of)
        .collect()
}

/// The link title of an anchor, falling back to its text.
pub fn link_title(anchor: ElementRef<'_>) -> String {
    anchor
        .value()
        .attr("title")
        .map(str::to_owned)
        .unwrap_or_else(|| text_of(anchor))
}

/// Whether an anchor points at a wiki article (not a file, category or
/// external link).
pub fn is_article_link(anchor: ElementRef<'_>) -> bool {
    let Some(href) = anchor.value().attr("href") else {
        return false;
    };
    href.starts_with("/wiki/")
        && !href.starts_with("/wiki/File:")
        && !href.starts_with("/wiki/Category:")
        && !href.contains("redlink=1")
}

/// A hash of text with whitespace collapsed, so reflowed HTML does not count
/// as a change (T2.4.5). BLAKE3, prefixed with the algorithm so a later
/// change of algorithm is visible in the data.
pub fn text_hash(text: &str) -> String {
    let normalised = collapse(text);
    format!(
        "blake3:{}",
        &blake3::hash(normalised.as_bytes()).to_hex()[..32]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_including_nbsp_collapses() {
        assert_eq!(collapse("  Sacrifice\u{a0}%\n "), "Sacrifice %");
        assert_eq!(collapse("a\t\tb"), "a b");
    }

    #[test]
    fn a_hash_ignores_reflowing_and_names_its_algorithm() {
        assert_eq!(text_hash("Lorem  ipsum\n"), text_hash("Lorem ipsum"));
        assert_ne!(text_hash("Lorem ipsum"), text_hash("Lorem ipsam"));
        assert!(text_hash("x").starts_with("blake3:"));
    }

    #[test]
    fn headings_and_redirect_markers_are_found() {
        let body = r#"<html><body><h1 id="firstHeading"><span>Energy Surge</span></h1>
            <div id="contentSub"><span class="mw-redirectedfrom">(Redirected from
            <a href="/wiki/ES?redirect=no" title="ES">ES</a>)</span></div></body></html>"#;
        assert_eq!(page_heading(body).as_deref(), Some("Energy Surge"));
        assert_eq!(redirected_from(body).as_deref(), Some("ES"));
        assert_eq!(redirected_from("<html></html>"), None);
    }
}
