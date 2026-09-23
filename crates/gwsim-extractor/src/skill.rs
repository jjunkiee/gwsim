//! Skill pages: from cached HTML to raw strings and numbers (T2.4.1, T2.4.3,
//! T2.4.5).
//!
//! [`parse_skill`] reads a page into a [`RawSkill`]: the infobox's fields as
//! the wiki writes them, the progression table as numbers, the categories,
//! and a hash of the description. [`crate::normalise`] turns that into the
//! typed numbers part of a skill file.
//!
//! Missing fields are `None`, never errors: pages vary, and `diff` must
//! tolerate a field disappearing (§22).

use scraper::{ElementRef, Html};

use crate::html::{categories, collapse, link_title, selector, text_hash, text_of};

/// One row of a progression table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressionRow {
    /// The row label as the wiki writes it, such as "Energy loss".
    pub label: String,
    /// `(rank, value)` for every column, in rank order.
    pub values: Vec<(u8, i32)>,
}

impl ProgressionRow {
    /// The value at a rank, if the table has that column.
    pub fn at(&self, rank: u8) -> Option<i32> {
        self.values
            .iter()
            .find(|(r, _)| *r == rank)
            .map(|(_, value)| *value)
    }
}

/// A skill page's progression table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progression {
    /// What the columns count: an attribute or a title rank.
    pub attribute_label: String,
    pub rows: Vec<ProgressionRow>,
}

/// Everything read from a skill page, before normalisation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RawSkill {
    /// The name in the infobox heading.
    pub name: String,
    /// The id from the infobox image's title (T2.1 §3).
    pub id: Option<u16>,
    /// The infobox's extra classes: a profession, empty, or `Monster Monster`.
    pub box_class: String,
    /// `(field, value)` from the stats list, in page order. The value is the
    /// hidden sort key where the page has one, so `¾` arrives as `0.75`.
    pub stats: Vec<(String, String)>,
    pub profession: Option<String>,
    pub attribute: Option<String>,
    /// The Type row, such as "Elite hex spell".
    pub type_text: Option<String>,
    /// The Type row carries "(PvE-only)".
    pub pve_only: bool,
    pub campaigns: Vec<String>,
    /// The Special row, such as "Monster skill" or "Resurrection skill".
    pub special: Option<String>,
    pub categories: Vec<String>,
    pub progression: Option<Progression>,
    /// A hash of the description, never the description (EXT-7).
    pub description_hash: Option<String>,
    /// The `x...y...z` values quoted in the description, as numbers only.
    /// Used to cross-check the progression table, never stored.
    pub description_values: Vec<Vec<i32>>,
}

impl RawSkill {
    /// A stat by field name, such as `Energy`.
    pub fn stat(&self, field: &str) -> Option<&str> {
        self.stats
            .iter()
            .find(|(name, _)| name == field)
            .map(|(_, value)| value.as_str())
    }

    /// Whether the page is in a category.
    pub fn in_category(&self, name: &str) -> bool {
        self.categories.iter().any(|category| category == name)
    }

    /// Whether this is a monster skill.
    pub fn is_monster(&self) -> bool {
        self.box_class.contains("Monster")
            || self.profession.as_deref() == Some("Monster")
            || self.special.as_deref() == Some("Monster skill")
    }
}

/// Parses a skill page. [`None`] if it has no skill infobox.
pub fn parse_skill(body: &str) -> Option<RawSkill> {
    let document = Html::parse_document(body);
    let infobox = document.select(&selector("div.infobox.skill-box")).next()?;

    let mut raw = RawSkill {
        name: infobox
            .select(&selector("p.skill-name"))
            .next()
            .map(text_of)
            .unwrap_or_default(),
        box_class: infobox
            .value()
            .classes()
            .filter(|class| *class != "infobox" && *class != "skill-box")
            .collect::<Vec<_>>()
            .join(" "),
        categories: categories(&document),
        ..RawSkill::default()
    };

    raw.id = infobox
        .select(&selector("div.skill-image a"))
        .next()
        .and_then(|anchor| anchor.value().attr("title"))
        .and_then(|title| title.trim().parse().ok());

    for item in infobox.select(&selector("div.skill-stats li")) {
        let Some(anchor) = item.select(&selector("a")).next() else {
            continue;
        };
        let field = anchor.value().attr("title").unwrap_or("").to_owned();
        let hidden = item
            .select(&selector("span"))
            .find(|span| {
                span.value()
                    .attr("style")
                    .is_some_and(|s| s.contains("display:none"))
            })
            .map(text_of);
        let value = hidden.unwrap_or_else(|| stat_text(item));
        raw.stats.push((field, value));
    }

    let terms: Vec<_> = infobox.select(&selector("dl dt")).collect();
    for term in terms {
        let label = text_of(term);
        let Some(definition) = term
            .next_siblings()
            .filter_map(ElementRef::wrap)
            .find(|element| element.value().name() == "dd")
        else {
            continue;
        };
        let value = text_of(definition);
        match label.as_str() {
            "Profession" => raw.profession = Some(value),
            "Attribute" => raw.attribute = Some(value),
            "Type" => {
                raw.pve_only = value.contains("PvE-only");
                let type_text = value.replace("(PvE-only)", "");
                raw.type_text = Some(collapse(&type_text));
            }
            "Campaign" | "Campaigns" => {
                raw.campaigns = definition.select(&selector("a")).map(link_title).collect();
                if raw.campaigns.is_empty() {
                    raw.campaigns = vec![value];
                }
            }
            "Special" => raw.special = Some(value),
            _ => {}
        }
    }

    if let Some(description) = document.select(&selector("div.noexcerpt p")).next() {
        raw.description_hash = Some(text_hash(&text_of(description)));
        raw.description_values = description
            .select(&selector("span"))
            .map(text_of)
            .filter_map(|text| parse_dotted(&text))
            .collect();
    }

    raw.progression = document
        .select(&selector("table.skill-progression"))
        .next()
        .and_then(parse_progression);

    Some(raw)
}

/// The text of a stat item before its icon link.
fn stat_text(item: ElementRef<'_>) -> String {
    let mut text = String::new();
    for node in item.children() {
        if let Some(element) = ElementRef::wrap(node) {
            if element.value().name() == "a" {
                break;
            }
            text.push_str(&element.text().collect::<String>());
        } else if let Some(fragment) = node.value().as_text() {
            text.push_str(fragment);
        }
    }
    collapse(&text)
}

/// `1...8...10` as numbers. Percentages and decimals do not qualify.
fn parse_dotted(text: &str) -> Option<Vec<i32>> {
    if !text.contains("...") {
        return None;
    }
    text.split("...")
        .map(|part| part.trim().parse().ok())
        .collect()
}

fn parse_progression(table: ElementRef<'_>) -> Option<Progression> {
    let cells: Vec<_> = table.select(&selector("td")).collect();
    let labels_cell = cells.first()?;
    let div = selector("div");
    let mut labels = labels_cell.select(&div);
    let attribute_label = labels.next().map(text_of)?;
    let row_labels: Vec<String> = labels.map(text_of).collect();

    let mut rows: Vec<ProgressionRow> = row_labels
        .into_iter()
        .map(|label| ProgressionRow {
            label,
            values: Vec::new(),
        })
        .collect();

    for column in table.select(&selector("div.column")) {
        let mut divs = column.select(&div);
        let Some(rank) = divs.next().and_then(|d| text_of(d).parse::<u8>().ok()) else {
            continue;
        };
        for (row, value) in rows.iter_mut().zip(divs) {
            if let Ok(number) = text_of(value).parse::<i32>() {
                row.values.push((rank, number));
            }
        }
    }
    Some(Progression {
        attribute_label,
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotted_values_parse_only_when_they_are_whole_numbers() {
        assert_eq!(parse_dotted("1...8...10"), Some(vec![1, 8, 10]));
        assert_eq!(parse_dotted("20...30"), Some(vec![20, 30]));
        assert_eq!(parse_dotted("6.6...14.85...16.5%"), None);
        assert_eq!(parse_dotted("7"), None);
    }
}
