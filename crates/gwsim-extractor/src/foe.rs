//! Foe and area pages (T2.3.3, WP2.5).
//!
//! A foe page is an `{{NPC infobox}}` table followed by Skills and Armor
//! ratings sections; an area page lists its foes under a `Foes` heading
//! (T2.1 §3). Both parsers return raw facts: names, numbers and flags. Notes
//! such as "(unusable with sword)" are read only to set a flag and are never
//! kept (EXT-7).

use gwsim_data::core::{Attribute, DamageType, Profession};
use gwsim_data::foe::{CreatureTrait, ModeValue};
use scraper::{ElementRef, Html};

use crate::discovery::{parse_attribute, parse_profession};
use crate::html::{is_article_link, link_title, selector, text_of};

/// Every `NM (HM)` pair in a text, in order. A bare number is a pair with no
/// hard-mode value.
///
/// "16 (25), 20 (26)" gives two pairs; "20" gives one with `hm: None`.
pub fn parse_level_pairs(text: &str) -> Vec<ModeValue<u8>> {
    let mut pairs = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        if !chars[index].is_ascii_digit() {
            index += 1;
            continue;
        }
        let (nm, next) = read_number(&chars, index);
        index = next;
        let mut look = index;
        while look < chars.len() && chars[look] == ' ' {
            look += 1;
        }
        let mut hm = None;
        if look < chars.len()
            && chars[look] == '('
            && chars.get(look + 1).is_some_and(char::is_ascii_digit)
        {
            let (value, after) = read_number(&chars, look + 1);
            if chars.get(after) == Some(&')') {
                hm = Some(value);
                index = after + 1;
            }
        }
        if let Ok(nm) = u8::try_from(nm) {
            pairs.push(ModeValue {
                nm,
                hm: hm.and_then(|value| u8::try_from(value).ok()),
            });
        }
    }
    pairs
}

fn read_number(chars: &[char], start: usize) -> (u32, usize) {
    let mut end = start;
    let mut value = 0u32;
    while end < chars.len() && chars[end].is_ascii_digit() {
        value = value * 10 + chars[end].to_digit(10).unwrap_or(0);
        end += 1;
    }
    (value, end)
}

/// Attribute ranks from a Skills-section line.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AttributeText {
    pub nm: Vec<(Attribute, u8)>,
    /// Only the ranks the page says change in hard mode. [`None`] when it
    /// says nothing about hard mode, which A-005 later fills (T1.4.8).
    pub hm: Option<Vec<(Attribute, u8)>>,
    /// Attribute names that did not parse.
    pub unknown: Vec<String>,
}

/// Parses "15 Domination Magic, 14 Inspiration Magic (20 Domination Magic in
/// Hard mode)".
pub fn parse_attribute_text(text: &str) -> AttributeText {
    let mut result = AttributeText::default();
    let (normal, hard) = match text.find('(') {
        Some(open) if text[open..].to_lowercase().contains("hard mode") => {
            let close = text[open..]
                .find(')')
                .map(|c| open + c)
                .unwrap_or(text.len());
            (&text[..open], Some(&text[open + 1..close]))
        }
        _ => (text, None),
    };

    result.nm = rank_list(normal, &mut result.unknown);
    if let Some(hard) = hard {
        let lower = hard.to_lowercase();
        let cut = lower.find("in hard mode").unwrap_or(hard.len());
        let changed = rank_list(&hard[..cut], &mut result.unknown);
        // The page lists only what changes; the rest keep their normal rank.
        let mut ranks = result.nm.clone();
        for (attribute, rank) in changed {
            match ranks.iter_mut().find(|(a, _)| *a == attribute) {
                Some(entry) => entry.1 = rank,
                None => ranks.push((attribute, rank)),
            }
        }
        result.hm = Some(ranks);
    }
    result
}

fn rank_list(text: &str, unknown: &mut Vec<String>) -> Vec<(Attribute, u8)> {
    let mut ranks = Vec::new();
    for part in text.split([',', ';']).flat_map(|p| p.split(" and ")) {
        let part = part.trim();
        let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
        let Ok(rank) = digits.parse::<u8>() else {
            continue;
        };
        let name = part[digits.len()..].trim();
        match parse_attribute(name) {
            Some(attribute) => ranks.push((attribute, rank)),
            None if !name.is_empty() => unknown.push(name.to_owned()),
            None => {}
        }
    }
    ranks
}

/// One skill on a foe's bar, as the page lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawFoeSkill {
    pub title: String,
    /// Marked "(Hard mode only)".
    pub hm_only: bool,
    /// "level 20 and above only": the lowest level that carries it.
    pub min_level: Option<u8>,
    /// Marked "(elite, …)".
    pub elite: bool,
}

/// One loadout. A page without variant headings has one, unnamed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RawVariant {
    /// The variant heading, such as "Axe-wielder".
    pub name: Option<String>,
    /// The levels a heading such as "Level 20 and above" limits this loadout
    /// to: `(lowest, highest)`, where `None` is open-ended. [`None`] for a
    /// loadout variant such as "Hammer-wielder".
    pub levels: Option<(u8, Option<u8>)>,
    pub attributes: AttributeText,
    pub skills: Vec<RawFoeSkill>,
}

impl RawVariant {
    /// Whether a level-qualified variant covers a level.
    pub fn covers(&self, level: u8) -> bool {
        self.levels
            .is_some_and(|(low, high)| level >= low && high.is_none_or(|high| level <= high))
    }
}

/// The level range a variant heading names, if it names one.
///
/// "Level 16" is exactly 16; "Levels 20 and above" is 20 upwards.
pub fn heading_levels(heading: &str) -> Option<(u8, Option<u8>)> {
    let lower = heading.to_lowercase();
    if !lower.starts_with("level") {
        return None;
    }
    let digits: String = lower
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect();
    let low: u8 = digits.parse().ok()?;
    if lower.contains("and above") || lower.contains("and higher") {
        Some((low, None))
    } else {
        Some((low, Some(low)))
    }
}

/// Splits a line such as "14 Expertise at level 20" or "Level 20: 15 Divine
/// Favor" into the level it is for and the ranks.
fn level_qualified(line: &str) -> (Option<u8>, String) {
    let lower = line.to_lowercase();
    if let Some(rest) = lower.strip_prefix("level ") {
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if let (Ok(level), Some(colon)) = (digits.parse::<u8>(), line.find(':')) {
            return (Some(level), line[colon + 1..].trim().to_owned());
        }
    }
    if let Some(at) = lower.find(" at level ") {
        let digits: String = lower[at + 10..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if let Ok(level) = digits.parse::<u8>() {
            let rest = &line[at + 10 + digits.len()..];
            return (Some(level), format!("{}{}", &line[..at], rest));
        }
    }
    (None, line.to_owned())
}

/// The attribute ranks in a paragraph, choosing the line for the highest
/// level it mentions (the level M1's foes are fought at).
fn attribute_paragraph(paragraph: ElementRef<'_>) -> Option<AttributeText> {
    let html = paragraph.inner_html();
    let mut best: Option<(Option<u8>, AttributeText)> = None;
    for (index, piece) in html.split("<br").enumerate() {
        // Every piece after the first starts with the rest of the `<br />`.
        let piece = if index == 0 {
            piece
        } else {
            piece.split_once('>').map(|(_, rest)| rest).unwrap_or("")
        };
        let text = crate::html::collapse(
            &Html::parse_fragment(piece)
                .root_element()
                .text()
                .collect::<String>(),
        );
        let (level, rest) = level_qualified(&text);
        let parsed = parse_attribute_text(&rest);
        if parsed.nm.is_empty() {
            continue;
        }
        let better = match &best {
            None => true,
            Some((best_level, _)) => level.unwrap_or(0) >= best_level.unwrap_or(0),
        };
        if better {
            best = Some((level, parsed));
        }
    }
    best.map(|(_, parsed)| parsed)
}

/// One armor table.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RawArmorTable {
    /// The paragraph before the table, such as "Hammer wielder".
    pub label: Option<String>,
    /// The level the header says the figures are for.
    pub level: Option<u8>,
    pub values: Vec<(DamageType, i16)>,
}

/// Everything read from a foe page.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RawFoe {
    pub name: String,
    pub affiliation: Option<String>,
    pub species: Option<String>,
    pub professions: Vec<Profession>,
    pub levels: Vec<ModeValue<u8>>,
    pub boss: bool,
    pub variants: Vec<RawVariant>,
    pub armor: Vec<RawArmorTable>,
    /// Anything that did not parse, for the report.
    pub warnings: Vec<String>,
}

impl RawFoe {
    /// The highest level pair, which is the one an area's elite spawns use.
    pub fn top_level(&self) -> Option<&ModeValue<u8>> {
        self.levels.iter().max_by_key(|pair| pair.nm)
    }
}

/// The creature traits a species implies, where the mapping is known.
pub fn traits_for_species(species: &str) -> Option<Vec<CreatureTrait>> {
    use CreatureTrait as T;
    Some(match species {
        "Human" | "Dwarf" | "Centaur" | "Charr" | "Heket" | "Margonite" | "Tengu" | "Norn"
        | "Asura" | "Kurzick" | "Luxon" | "Naga" | "Ogre" | "Giant" | "Mandragor" => {
            vec![T::Fleshy]
        }
        "Beast" | "Animal" => vec![T::Fleshy, T::Animal],
        "Demon" => vec![T::Fleshy, T::Demon],
        "Plant" => vec![T::Plant],
        "Undead" => vec![T::Undead],
        "Spirit" => vec![T::Spirit],
        "Construct" | "Golem" => vec![T::Construct],
        "Elemental" | "Djinn" => vec![T::Elemental],
        _ => return None,
    })
}

/// Parses an NPC page.
pub fn parse_foe(body: &str) -> Option<RawFoe> {
    let document = Html::parse_document(body);
    let infobox = document.select(&selector("table.npc-infobox")).next()?;
    let mut foe = RawFoe {
        name: infobox
            .select(&selector("th"))
            .next()
            .map(text_of)
            .unwrap_or_default(),
        ..RawFoe::default()
    };

    for row in infobox.select(&selector("tr")) {
        let (Some(header), Some(cell)) = (
            row.select(&selector("th")).next(),
            row.select(&selector("td")).next(),
        ) else {
            continue;
        };
        let label = text_of(header);
        match label.as_str() {
            "Affiliation" => foe.affiliation = Some(text_of(cell)),
            "Type" => foe.species = Some(text_of(cell)),
            "Profession" | "Professions" => {
                for anchor in cell.select(&selector("a")).filter(|a| is_article_link(*a)) {
                    let name = link_title(anchor);
                    match parse_profession(&name) {
                        Some(profession) if !foe.professions.contains(&profession) => {
                            foe.professions.push(profession)
                        }
                        Some(_) => {}
                        None => foe.warnings.push(format!("unknown profession {name:?}")),
                    }
                }
            }
            "Level(s)" | "Level" | "Levels" => foe.levels = parse_level_pairs(&text_of(cell)),
            "Boss" => foe.boss = true,
            _ => {}
        }
    }
    if body.contains("title=\"Boss\"") && infobox.html().contains("title=\"Boss\"") {
        foe.boss = true;
    }

    parse_skills_section(&document, &mut foe);
    parse_armor_section(&document, &mut foe);
    Some(foe)
}

/// The siblings after a section heading, up to the next heading of the same
/// or a higher level.
fn section_after<'a>(document: &'a Html, id: &str, level: u8) -> Vec<ElementRef<'a>> {
    let Some(headline) = document
        .select(&selector(&format!("span.mw-headline[id=\"{id}\"]")))
        .next()
    else {
        return Vec::new();
    };
    let Some(heading) = headline.parent().and_then(ElementRef::wrap) else {
        return Vec::new();
    };
    let mut elements = Vec::new();
    for sibling in heading.next_siblings().filter_map(ElementRef::wrap) {
        let name = sibling.value().name();
        if let Some(rank) = name.strip_prefix('h').and_then(|d| d.parse::<u8>().ok())
            && rank <= level
        {
            break;
        }
        elements.push(sibling);
    }
    elements
}

/// A heading's own text, without the `[edit]` link every real heading
/// carries.
fn heading_text(heading: ElementRef<'_>) -> String {
    heading
        .select(&selector("span.mw-headline"))
        .next()
        .map(text_of)
        .unwrap_or_else(|| text_of(heading))
}

fn parse_skills_section(document: &Html, foe: &mut RawFoe) {
    let mut current = RawVariant::default();
    let mut started = false;

    for element in section_after(document, "Skills", 2) {
        match element.value().name() {
            "h3" => {
                if started {
                    foe.variants.push(std::mem::take(&mut current));
                }
                let name = heading_text(element);
                current.levels = heading_levels(&name);
                current.name = Some(name);
                started = true;
            }
            "p" => {
                if let Some(parsed) = attribute_paragraph(element) {
                    for name in &parsed.unknown {
                        foe.warnings.push(format!("unknown attribute {name:?}"));
                    }
                    current.attributes = parsed;
                    started = true;
                }
            }
            "ul" => {
                for item in element.select(&selector("li")) {
                    let Some(anchor) = item.select(&selector("a")).find(|a| is_article_link(*a))
                    else {
                        continue;
                    };
                    let note = text_of(item).to_lowercase();
                    let min_level = note
                        .find("level ")
                        .and_then(|at| note[at + 6..].split_whitespace().next())
                        .and_then(|word| word.parse::<u8>().ok())
                        .filter(|_| note.contains("and above"));
                    current.skills.push(RawFoeSkill {
                        title: link_title(anchor),
                        hm_only: note.contains("hard mode only"),
                        min_level,
                        elite: note.contains("(elite"),
                    });
                }
                started = true;
            }
            _ => {}
        }
    }
    if started {
        foe.variants.push(current);
    }

    // A variant heading with no attribute line shares the first variant's
    // ranks: the page states them once for the whole foe.
    if let Some(first) = foe.variants.first().map(|v| v.attributes.clone()) {
        for variant in foe.variants.iter_mut().skip(1) {
            if variant.attributes.nm.is_empty() {
                variant.attributes = first.clone();
            }
        }
    }
}

fn parse_armor_section(document: &Html, foe: &mut RawFoe) {
    let mut label: Option<String> = None;
    for element in section_after(document, "Armor_ratings", 2) {
        match element.value().name() {
            "p" => {
                let text = text_of(element);
                if !text.is_empty() {
                    label = Some(text);
                }
            }
            "table" if element.value().classes().any(|c| c == "npc-statistics") => {
                foe.armor.push(parse_armor_table(element, label.take()));
            }
            _ => {}
        }
    }
}

fn parse_armor_table(table: ElementRef<'_>, label: Option<String>) -> RawArmorTable {
    let mut parsed = RawArmorTable {
        label,
        ..RawArmorTable::default()
    };
    if let Some(header) = table.select(&selector("th")).next() {
        let text = text_of(header);
        parsed.level = text
            .split("level")
            .nth(1)
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|word| word.parse().ok());
    }
    let cells: Vec<_> = table.select(&selector("td")).collect();
    for pair in cells.windows(2) {
        let Some(anchor) = pair[0].select(&selector("a")).next() else {
            continue;
        };
        let kind = link_title(anchor);
        let Some(name) = kind.strip_suffix(" damage") else {
            continue;
        };
        let damage = DamageType::ALL
            .into_iter()
            .find(|d| format!("{d:?}").eq_ignore_ascii_case(name));
        if let (Some(damage), Ok(value)) = (damage, text_of(pair[1]).parse::<i16>()) {
            parsed.values.push((damage, value));
        }
    }
    parsed
}

// ------------------------------------------------------------------ areas

/// One foe row in an area's list.
#[derive(Debug, Clone, PartialEq)]
pub struct RosterFoe {
    pub title: String,
    pub profession: Option<Profession>,
    pub level: Option<ModeValue<u8>>,
}

/// A group heading and its foes, such as "Humans (Kournan military)".
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RosterGroup {
    pub name: String,
    pub foes: Vec<RosterFoe>,
}

/// An area's foe roster (T2.5.4). Kept for findings and encounter authoring
/// only: group composition is hand-authored (A-006).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AreaRoster {
    pub area: String,
    pub groups: Vec<RosterGroup>,
    pub bosses: Vec<RosterGroup>,
}

impl AreaRoster {
    /// Every non-boss foe title, in page order.
    pub fn foe_titles(&self) -> Vec<String> {
        self.groups
            .iter()
            .flat_map(|group| group.foes.iter().map(|foe| foe.title.clone()))
            .collect()
    }
}

/// Parses an explorable area page's Foes and Bosses sections.
pub fn parse_area(body: &str) -> AreaRoster {
    let document = Html::parse_document(body);
    let mut roster = AreaRoster {
        area: crate::html::page_heading(body).unwrap_or_default(),
        ..AreaRoster::default()
    };

    let mut bosses = false;
    let mut group = RosterGroup::default();
    let flush = |group: &mut RosterGroup, into: &mut Vec<RosterGroup>| {
        if !group.foes.is_empty() {
            into.push(std::mem::take(group));
        } else {
            group.name.clear();
        }
    };

    for element in section_after(&document, "Foes", 3) {
        match element.value().name() {
            "h4" => {
                let target = if bosses {
                    &mut roster.bosses
                } else {
                    &mut roster.groups
                };
                flush(&mut group, target);
                let heading = heading_text(element);
                if heading == "Bosses" {
                    bosses = true;
                } else {
                    break;
                }
            }
            "p" => {
                let target = if bosses {
                    &mut roster.bosses
                } else {
                    &mut roster.groups
                };
                flush(&mut group, target);
                group.name = text_of(element);
            }
            "ul" => {
                for item in element.select(&selector("li")) {
                    let profession = item
                        .select(&selector("a.image"))
                        .next()
                        .and_then(|a| a.value().attr("title"))
                        .and_then(parse_profession);
                    let Some(anchor) = item.select(&selector("a")).find(|a| is_article_link(*a))
                    else {
                        continue;
                    };
                    let text = text_of(item);
                    let name = text_of(anchor);
                    let before = text.split(name.as_str()).next().unwrap_or("");
                    group.foes.push(RosterFoe {
                        title: link_title(anchor),
                        profession,
                        level: parse_level_pairs(before).into_iter().next(),
                    });
                }
            }
            _ => {}
        }
    }
    let target = if bosses {
        &mut roster.bosses
    } else {
        &mut roster.groups
    };
    flush(&mut group, target);
    roster
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_pairs_parse_in_every_shape() {
        let pairs = parse_level_pairs("16 (25), 20 (26)");
        assert_eq!(pairs.len(), 2);
        assert_eq!((pairs[1].nm, pairs[1].hm), (20, Some(26)));
        let bare = parse_level_pairs("20");
        assert_eq!((bare[0].nm, bare[0].hm), (20, None));
        assert!(parse_level_pairs("").is_empty());
    }

    #[test]
    fn attribute_text_gives_normal_and_hard_mode_ranks() {
        let ranks = parse_attribute_text(
            "15 Domination Magic, 14 Inspiration Magic (20 Domination Magic in Hard mode)",
        );
        assert_eq!(
            ranks.nm,
            vec![
                (Attribute::DominationMagic, 15),
                (Attribute::InspirationMagic, 14)
            ]
        );
        assert_eq!(
            ranks.hm,
            Some(vec![
                (Attribute::DominationMagic, 20),
                (Attribute::InspirationMagic, 14)
            ])
        );

        let plain = parse_attribute_text("14 Strength");
        assert_eq!(plain.nm, vec![(Attribute::Strength, 14)]);
        assert_eq!(plain.hm, None, "silence about hard mode is left for A-005");
    }

    #[test]
    fn species_map_to_traits_or_say_they_do_not() {
        assert_eq!(
            traits_for_species("Human"),
            Some(vec![CreatureTrait::Fleshy])
        );
        assert_eq!(
            traits_for_species("Undead"),
            Some(vec![CreatureTrait::Undead])
        );
        assert_eq!(traits_for_species("Something new"), None);
    }
}
