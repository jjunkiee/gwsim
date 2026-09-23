//! Finding every skill through the wiki's list pages (WP2.3).
//!
//! Category pagination is disallowed (C1), so discovery reads article pages
//! that list everything instead (T2.1 §5):
//!
//! - `Skill template format/Skill list` for every player skill's id;
//! - the eleven "List of … skills" pages for profession, attribute, campaign
//!   and elite;
//! - "List of PvE-only skills" for that flag;
//! - the game-integration pages, which also cover monster skills, as a
//!   cross-check.
//!
//! The list pages carry prose descriptions. They are read only to find
//! titles and flags; no description text leaves this module (EXT-7).

use std::collections::{BTreeMap, BTreeSet};

use gwsim_data::core::{Attribute, Campaign, Profession};
use gwsim_data::skill_index::IndexedSkill;
use gwsim_data::{SkillId, WikiTitle, slugify};
use scraper::Html;

use crate::html::{collapse, is_article_link, link_title, selector, text_of};

/// The page listing every player-loadable skill with its id.
pub const SKILL_LIST: &str = "Skill template format/Skill list";
/// The page listing every PvE-only skill.
pub const PVE_ONLY_LIST: &str = "List of PvE-only skills";
/// The game-integration pages that exist, by id range.
pub const GAME_INTEGRATION_RANGES: [&str; 7] = [
    "1-500",
    "501-1000",
    "1001-1500",
    "1501-2000",
    "2001-2500",
    "2501-3000",
    "3001-3500",
];

/// The game-integration page for an id range.
pub fn game_integration_title(range: &str) -> WikiTitle {
    WikiTitle(format!("Guild Wars Wiki:Game integration/Skills/{range}"))
}

/// A profession's skill list page. [`None`] is the common skills list.
///
/// The titles are lower case ("List of mesmer skills"); the capitalised
/// spelling the plan used is a 404 (T2.1 §2).
pub fn profession_list_title(profession: Option<Profession>) -> WikiTitle {
    let name = match profession {
        Some(profession) => format!("{profession:?}").to_lowercase(),
        None => "common".to_owned(),
    };
    WikiTitle(format!("List of {name} skills"))
}

/// Every page skill discovery reads, in crawl order. About 20 pages.
pub fn discovery_titles() -> Vec<WikiTitle> {
    let mut titles = vec![
        WikiTitle(SKILL_LIST.to_owned()),
        WikiTitle(PVE_ONLY_LIST.to_owned()),
    ];
    titles.extend(
        Profession::ALL
            .iter()
            .map(|p| profession_list_title(Some(*p))),
    );
    titles.push(profession_list_title(None));
    titles.extend(
        GAME_INTEGRATION_RANGES
            .iter()
            .map(|r| game_integration_title(r)),
    );
    titles
}

// ------------------------------------------------------------- vocabulary

/// Parses a profession name as the wiki writes it.
pub fn parse_profession(text: &str) -> Option<Profession> {
    let wanted = squash(text);
    Profession::ALL
        .into_iter()
        .find(|profession| squash(&format!("{profession:?}")) == wanted)
}

/// Parses an attribute name as the wiki writes it ("Domination Magic").
pub fn parse_attribute(text: &str) -> Option<Attribute> {
    let wanted = squash(text);
    Attribute::ALL
        .into_iter()
        .find(|attribute| squash(&format!("{attribute:?}")) == wanted)
}

/// Parses a campaign name as the wiki writes it ("Eye of the North").
pub fn parse_campaign(text: &str) -> Option<Campaign> {
    let wanted = squash(text);
    Campaign::ALL
        .into_iter()
        .find(|campaign| squash(&format!("{campaign:?}")) == wanted)
}

/// Lower case, with spaces, hyphens and apostrophes removed, so wiki names
/// and Rust variant names compare equal.
pub fn squash(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Whether a title is the PvP half of a split skill (D3: not modelled).
pub fn is_pvp_title(title: &str) -> bool {
    title.trim_end().ends_with("(PvP)")
}

/// The page title behind an allegiance skill's list name.
///
/// The Kurzick and Luxon versions of a Factions allegiance skill have two
/// ids and two names in the skill list ("Shadow Sanctuary (Luxon)") but one
/// wiki page ("Shadow Sanctuary"), which is the name the profession lists
/// use (T2.3.6).
pub fn allegiance_base(title: &str) -> &str {
    title
        .strip_suffix(" (Luxon)")
        .or_else(|| title.strip_suffix(" (Kurzick)"))
        .unwrap_or(title)
}

// ---------------------------------------------------------------- parsers

/// One row of the skill list: an id and its page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedSkill {
    pub id: SkillId,
    pub title: WikiTitle,
}

/// Parses `Skill template format/Skill list`.
///
/// Skips id 0, "No Skill", which is a template placeholder, not a skill.
pub fn parse_skill_list(body: &str) -> Vec<ListedSkill> {
    let document = Html::parse_document(body);
    let mut skills = Vec::new();
    for row in document.select(&selector("table.sortable tr")) {
        let cells: Vec<_> = row.select(&selector("td")).collect();
        if cells.len() < 2 {
            continue;
        }
        let Ok(id) = text_of(cells[0]).parse::<u16>() else {
            continue;
        };
        let Some(link) = cells[1]
            .select(&selector("a"))
            .find(|a| is_article_link(*a))
        else {
            continue;
        };
        if id == 0 {
            continue;
        }
        skills.push(ListedSkill {
            id: SkillId(id),
            title: WikiTitle(link_title(link)),
        });
    }
    skills
}

/// One row of a game-integration page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameIntegrationRow {
    pub id: SkillId,
    /// [`None`] for an id the game does not use.
    pub title: Option<WikiTitle>,
}

/// A parsed game-integration page.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GameIntegrationPage {
    pub rows: Vec<GameIntegrationRow>,
    /// A link to a following range page, which means a newer page exists
    /// than [`GAME_INTEGRATION_RANGES`] knows about.
    pub next_page: Option<WikiTitle>,
}

/// Parses a `Guild Wars Wiki:Game integration/Skills/<range>` page.
pub fn parse_game_integration(body: &str) -> GameIntegrationPage {
    let document = Html::parse_document(body);
    let mut page = GameIntegrationPage::default();

    for item in document.select(&selector(".mw-parser-output li")) {
        let text = text_of(item);
        let Some(rest) = text.strip_prefix("Skill ") else {
            continue;
        };
        let number: String = rest.chars().take_while(char::is_ascii_digit).collect();
        let Ok(id) = number.parse::<u16>() else {
            continue;
        };
        let title = item
            .select(&selector("a"))
            .find(|a| is_article_link(*a))
            .map(|a| WikiTitle(link_title(a)));
        page.rows.push(GameIntegrationRow {
            id: SkillId(id),
            title,
        });
    }

    let known: BTreeSet<String> = GAME_INTEGRATION_RANGES
        .iter()
        .map(|range| game_integration_title(range).0)
        .collect();
    page.next_page = document
        .select(&selector(".mw-parser-output a"))
        .map(link_title)
        .filter(|title| title.starts_with("Guild Wars Wiki:Game integration/Skills/"))
        .find(|title| !known.contains(title))
        .map(WikiTitle);
    page
}

/// One row of a profession's skill list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfessionRow {
    pub title: WikiTitle,
    pub profession: Option<Profession>,
    /// The attribute column's text; [`None`] for "No Attribute".
    pub attribute: Option<String>,
    pub elite: bool,
    pub campaign: Option<Campaign>,
    /// The campaign as written, kept so unknown campaigns can be reported.
    pub campaign_text: String,
}

/// Parses a "List of … skills" page.
///
/// Elite skills have a gold name cell (T2.1 §3). The description column is
/// read only to confirm that, never kept.
pub fn parse_profession_list(body: &str, profession: Option<Profession>) -> Vec<ProfessionRow> {
    let document = Html::parse_document(body);
    let mut rows = Vec::new();

    for row in document.select(&selector("tr[data-name]")) {
        let Some(name_cell) = row.select(&selector("th")).nth(1) else {
            continue;
        };
        let Some(link) = name_cell
            .select(&selector("a"))
            .find(|a| is_article_link(*a))
        else {
            continue;
        };
        let gold = name_cell
            .value()
            .attr("style")
            .is_some_and(|style| style.contains("#FD0"));
        let cells: Vec<_> = row.select(&selector("td")).collect();
        let description_says_elite = cells
            .first()
            .is_some_and(|cell| text_of(*cell).starts_with("Elite "));

        // The last two cells are attribute and campaign.
        let campaign_text = cells.last().map(|cell| text_of(*cell)).unwrap_or_default();
        let attribute_text = cells
            .len()
            .checked_sub(2)
            .and_then(|index| cells.get(index))
            .map(|cell| {
                // The cell carries a hidden sort key after the name.
                let text = text_of(*cell);
                text.strip_suffix('Z')
                    .map(str::trim)
                    .unwrap_or(&text)
                    .to_owned()
            })
            .unwrap_or_default();

        rows.push(ProfessionRow {
            title: WikiTitle(link_title(link)),
            profession,
            attribute: match attribute_text.as_str() {
                "" | "No Attribute" | "No attribute" => None,
                _ => Some(attribute_text),
            },
            elite: gold || description_says_elite,
            campaign: parse_campaign(&campaign_text),
            campaign_text,
        });
    }
    rows
}

/// The titles of every `tr[data-name]` row, which is how the PvE-only list
/// names its skills.
pub fn parse_named_rows(body: &str) -> BTreeSet<String> {
    let document = Html::parse_document(body);
    document
        .select(&selector("tr[data-name]"))
        .filter_map(|row| row.value().attr("data-name"))
        .map(collapse)
        .collect()
}

/// The members listed on a category's first page (at most 200).
pub fn parse_category_members(body: &str) -> Vec<WikiTitle> {
    let document = Html::parse_document(body);
    document
        .select(&selector("#mw-pages .mw-category-group li a"))
        .map(|a| WikiTitle(link_title(a)))
        .collect()
}

// ------------------------------------------------------------------ merge

/// What discovery found that does not fit neatly (T2.3.2 step 3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiscoveryReport {
    /// On a profession list but not in the skill list, so no id is known.
    /// Resolved later from the skill page's own infobox (T2.1 §3).
    pub titles_without_id: Vec<WikiTitle>,
    /// In the skill list but on no profession or PvE-only list.
    pub ids_without_list_entry: Vec<ListedSkill>,
    /// `(PvP)` titles, excluded under D3.
    pub pvp_excluded: Vec<ListedSkill>,
    /// Ids the game-integration pages name that the skill list does not:
    /// monster skills, mostly, kept apart from the player index.
    pub monster_or_unlisted: Vec<GameIntegrationRow>,
    /// List entries whose campaign gwsim has no variant for.
    pub unknown_campaigns: Vec<(WikiTitle, String)>,
    /// Titles the two id sources disagree about.
    pub id_conflicts: Vec<(SkillId, WikiTitle, WikiTitle)>,
}

/// The merged player skill index and what did not fit.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Merged {
    pub skills: Vec<IndexedSkill>,
    pub report: DiscoveryReport,
}

/// Joins the id list, the profession lists and the PvE-only list by title.
pub fn merge(
    listed: &[ListedSkill],
    professions: &[ProfessionRow],
    pve_only: &BTreeSet<String>,
    game_integration: &[GameIntegrationRow],
) -> Merged {
    let mut merged = Merged::default();
    let by_title: BTreeMap<&str, &ProfessionRow> = professions
        .iter()
        .map(|row| (row.title.as_str(), row))
        .collect();
    let listed_titles: BTreeSet<&str> = listed
        .iter()
        .map(|s| allegiance_base(s.title.as_str()))
        .collect();

    for skill in listed {
        if is_pvp_title(skill.title.as_str()) {
            merged.report.pvp_excluded.push(skill.clone());
            continue;
        }
        let base = allegiance_base(skill.title.as_str());
        let row = by_title
            .get(skill.title.as_str())
            .or_else(|| by_title.get(base));
        let is_pve_only = pve_only.contains(skill.title.as_str()) || pve_only.contains(base);
        if row.is_none() && !is_pve_only {
            merged.report.ids_without_list_entry.push(skill.clone());
        }
        if let Some(row) = row
            && row.campaign.is_none()
        {
            merged
                .report
                .unknown_campaigns
                .push((row.title.clone(), row.campaign_text.clone()));
        }
        merged.skills.push(IndexedSkill {
            id: skill.id,
            title: skill.title.clone(),
            slug: slugify(skill.title.as_str()),
            profession: row.and_then(|row| row.profession),
            elite: row.is_some_and(|row| row.elite),
            pve_only: is_pve_only,
            campaign: row.and_then(|row| row.campaign),
        });
    }

    for row in professions {
        if !listed_titles.contains(row.title.as_str()) && !is_pvp_title(row.title.as_str()) {
            merged.report.titles_without_id.push(row.title.clone());
        }
    }

    let by_id: BTreeMap<SkillId, &ListedSkill> = listed.iter().map(|s| (s.id, s)).collect();
    for row in game_integration {
        let Some(title) = &row.title else { continue };
        match by_id.get(&row.id) {
            None if !is_pvp_title(title.as_str()) => {
                merged.report.monster_or_unlisted.push(row.clone())
            }
            Some(listed) if allegiance_base(listed.title.as_str()) != title.as_str() => merged
                .report
                .id_conflicts
                .push((row.id, listed.title.clone(), title.clone())),
            _ => {}
        }
    }

    merged.skills.sort_by_key(|skill| skill.id);
    merged.skills.dedup_by_key(|skill| skill.id);
    merged
}

/// A category's first page compared with the merged index (T2.3.5 step 2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CategoryCheck {
    /// In the category but not in the index.
    pub missing_from_index: Vec<WikiTitle>,
    /// How many category members the index does have.
    pub matched: usize,
}

/// Checks a category's first page against the index. Only PvE titles are
/// expected in the index, so `(PvP)` members are ignored.
pub fn cross_check_category(members: &[WikiTitle], index: &[IndexedSkill]) -> CategoryCheck {
    let indexed: BTreeSet<&str> = index.iter().map(|s| s.title.as_str()).collect();
    let mut check = CategoryCheck::default();
    for member in members {
        if is_pvp_title(member.as_str()) {
            continue;
        }
        if indexed.contains(member.as_str()) {
            check.matched += 1;
        } else {
            check.missing_from_index.push(member.clone());
        }
    }
    check
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wiki_names_parse_to_the_data_vocabulary() {
        assert_eq!(parse_profession("Mesmer"), Some(Profession::Mesmer));
        assert_eq!(
            parse_attribute("Domination Magic"),
            Some(Attribute::DominationMagic)
        );
        assert_eq!(
            parse_attribute("Channeling Magic"),
            Some(Attribute::ChannelingMagic)
        );
        assert_eq!(
            parse_campaign("Eye of the North"),
            Some(Campaign::EyeOfTheNorth)
        );
        assert_eq!(parse_campaign("Core"), Some(Campaign::Core));
        assert_eq!(parse_campaign("Beyond"), None);
        assert_eq!(parse_profession("Monster"), None);
    }

    #[test]
    fn the_discovery_list_is_about_twenty_pages() {
        let titles = discovery_titles();
        assert_eq!(titles.len(), 2 + 10 + 1 + 7);
        assert!(titles.contains(&WikiTitle("List of mesmer skills".to_owned())));
        assert!(titles.contains(&WikiTitle("List of common skills".to_owned())));
    }

    #[test]
    fn pvp_titles_are_recognised() {
        assert!(is_pvp_title("Mistrust (PvP)"));
        assert!(!is_pvp_title("Mistrust"));
    }
}
