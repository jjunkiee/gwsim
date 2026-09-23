//! Game update pages, as context for the change report (T2.7.4).
//!
//! A `Feedback:Game_updates/<date>` page lists changes as `li` items: a
//! skill link, an optional `(BOTH)` or `(PvE)` marker, then ` - ` and the
//! change. The report attaches each line to the matching skill so a
//! maintainer sees why a number moved. The lines are shown in the local
//! report only and never written to `data/` (EXT-7).

use scraper::Html;

use crate::discovery::is_pvp_title;
use crate::html::{is_article_link, link_title, selector, text_of};

/// One change line naming a skill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateLine {
    /// The skill's page title.
    pub skill: String,
    /// The update page it came from.
    pub page: String,
    /// The change, as the update note words it.
    pub change: String,
}

/// Every PvE-relevant skill line on an update page.
pub fn parse_update(body: &str, page: &str) -> Vec<UpdateLine> {
    let document = Html::parse_document(body);
    let mut lines = Vec::new();
    for item in document.select(&selector(".mw-parser-output li")) {
        let Some(anchor) = item.select(&selector("a")).find(|a| is_article_link(*a)) else {
            continue;
        };
        let skill = link_title(anchor);
        if is_pvp_title(&skill) {
            continue;
        }
        let text = text_of(item);
        let Some((_, change)) = text.split_once(" - ") else {
            continue;
        };
        lines.push(UpdateLine {
            skill,
            page: page.to_owned(),
            change: change.trim().to_owned(),
        });
    }
    lines
}
