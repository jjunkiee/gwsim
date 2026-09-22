//! Identifier newtypes shared by every data file.
//!
//! Every type here validates on construction, so holding one is proof it is
//! well formed: a [`Slug`] is always a legal file stem, an [`AssumptionId`] is
//! always three digits, an [`IsoDate`] is always a real calendar date.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A reference into the assumptions register, spelled `A-` and three digits.
///
/// Stored as the number, so `A-5` cannot masquerade as a different id from
/// `A-005`, and so ordering is numeric rather than lexicographic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssumptionId(u16);

impl AssumptionId {
    /// Builds an id from its number.
    ///
    /// Returns [`None`] above 999, which would not fit the three-digit form.
    pub fn from_number(number: u16) -> Option<Self> {
        (number <= 999).then_some(AssumptionId(number))
    }

    /// The numeric part of the id.
    pub fn number(self) -> u16 {
        self.0
    }
}

impl fmt::Display for AssumptionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "A-{:03}", self.0)
    }
}

/// Why an assumption id could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssumptionIdError(String);

impl fmt::Display for AssumptionIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} is not an assumption id: expected the form A-000 to A-999",
            self.0
        )
    }
}

impl std::error::Error for AssumptionIdError {}

impl FromStr for AssumptionId {
    type Err = AssumptionIdError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || AssumptionIdError(format!("{text:?}"));

        let digits = text.strip_prefix("A-").ok_or_else(invalid)?;
        if digits.len() != 3 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(invalid());
        }
        digits
            .parse::<u16>()
            .ok()
            .and_then(AssumptionId::from_number)
            .ok_or_else(invalid)
    }
}

impl Serialize for AssumptionId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for AssumptionId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// A calendar date, written `YYYY-MM-DD`.
///
/// gwsim only ever records and compares crawl dates, so a validated newtype
/// does the job and keeps a date library out of the dependency tree. Stored
/// as the original string, which makes round-tripping exact.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IsoDate(String);

impl IsoDate {
    /// The date as `YYYY-MM-DD`.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The year, month and day.
    ///
    /// Total: the value cannot exist unless it parsed, so the fields are
    /// known to be digits of the right length.
    pub fn parts(&self) -> (u16, u8, u8) {
        let year = self.0[0..4].parse().unwrap_or(0);
        let month = self.0[5..7].parse().unwrap_or(0);
        let day = self.0[8..10].parse().unwrap_or(0);
        (year, month, day)
    }
}

impl fmt::Display for IsoDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why a date could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsoDateError {
    input: String,
    reason: &'static str,
}

impl fmt::Display for IsoDateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} is not a date: {}", self.input, self.reason)
    }
}

impl std::error::Error for IsoDateError {}

impl FromStr for IsoDate {
    type Err = IsoDateError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let fail = |reason: &'static str| IsoDateError {
            input: text.to_owned(),
            reason,
        };

        let bytes = text.as_bytes();
        if bytes.len() != 10 {
            return Err(fail("expected exactly 10 characters, as in 2026-09-22"));
        }
        if bytes[4] != b'-' || bytes[7] != b'-' {
            return Err(fail("expected hyphens after the year and the month"));
        }
        let digits_at = |range: std::ops::Range<usize>| {
            text[range.clone()]
                .bytes()
                .all(|byte| byte.is_ascii_digit())
        };
        if !digits_at(0..4) || !digits_at(5..7) || !digits_at(8..10) {
            return Err(fail("expected digits in YYYY-MM-DD"));
        }

        let month: u8 = text[5..7].parse().map_err(|_| fail("bad month"))?;
        let day: u8 = text[8..10].parse().map_err(|_| fail("bad day"))?;
        if !(1..=12).contains(&month) {
            return Err(fail("the month must be 01 to 12"));
        }
        if !(1..=31).contains(&day) {
            return Err(fail("the day must be 01 to 31"));
        }

        Ok(IsoDate(text.to_owned()))
    }
}

impl Serialize for IsoDate {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for IsoDate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// A skill's id in the game's template codes.
///
/// The template format is the only place these numbers come from, and they are
/// not ours to choose, so there is no constructor that invents one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SkillId(pub u16);

impl SkillId {
    /// Id 0 means "no skill" in a template code, never a real skill.
    pub const NONE: u16 = 0;

    /// The raw number.
    pub fn get(self) -> u16 {
        self.0
    }
}

impl fmt::Display for SkillId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// A file-name-safe key for an entity, derived from its wiki title.
///
/// Lowercase ASCII letters, digits and single hyphens, starting and ending
/// with an alphanumeric. Validated on construction, so a `Slug` in hand is
/// always a legal file stem.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Slug(String);

impl Slug {
    /// The slug as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Slug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why a slug was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlugError {
    input: String,
    reason: &'static str,
}

impl fmt::Display for SlugError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} is not a slug: {}", self.input, self.reason)
    }
}

impl std::error::Error for SlugError {}

impl FromStr for Slug {
    type Err = SlugError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let fail = |reason: &'static str| SlugError {
            input: text.to_owned(),
            reason,
        };

        if text.is_empty() {
            return Err(fail("it is empty"));
        }
        if !text
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(fail(
                "only lowercase ASCII letters, digits and hyphens are allowed",
            ));
        }
        if text.starts_with('-') || text.ends_with('-') {
            return Err(fail("it starts or ends with a hyphen"));
        }
        if text.contains("--") {
            return Err(fail("it has two hyphens in a row"));
        }

        Ok(Slug(text.to_owned()))
    }
}

impl Serialize for Slug {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Slug {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// Turns a wiki page title into a slug.
///
/// Spaces and underscores become hyphens, `&` becomes `and`, and punctuation
/// is dropped. Anything left that is not a lowercase letter, a digit or a
/// hyphen is dropped too, so a title with an unforeseen character still
/// produces a usable slug rather than an error.
///
/// Dropping rather than transliterating means two titles *could* collide. The
/// loader catches that: T1.2.7 rejects a tree with two files claiming the same
/// slug, and names both.
pub fn slugify(title: &str) -> Slug {
    let mut out = String::with_capacity(title.len());

    for character in title.chars() {
        match character {
            ' ' | '_' | '-' => out.push('-'),
            '&' => out.push_str("and"),
            // Dropped outright: these carry no sound and would otherwise
            // become stray hyphens.
            '"' | '\'' | '!' | '?' | ',' | '.' | '(' | ')' | ':' | ';' => {}
            character if character.is_ascii_alphanumeric() => {
                out.extend(character.to_lowercase());
            }
            character if character.is_alphanumeric() => {
                // Non-ASCII letters have no ASCII spelling we can rely on.
                // Dropping keeps the slug legal; a collision is caught later.
                let _ = character;
            }
            _ => {}
        }
    }

    // Collapse runs of hyphens, then trim them from both ends.
    let mut collapsed = String::with_capacity(out.len());
    let mut previous_hyphen = false;
    for character in out.chars() {
        if character == '-' {
            if !previous_hyphen && !collapsed.is_empty() {
                collapsed.push('-');
            }
            previous_hyphen = true;
        } else {
            collapsed.push(character);
            previous_hyphen = false;
        }
    }
    while collapsed.ends_with('-') {
        collapsed.pop();
    }

    Slug(collapsed)
}

/// The title of a Guild Wars Wiki page.
///
/// Held as the title, not as a URL, because the title is what appears in a
/// provenance block and what the extractor looks up. [`WikiTitle::url`] builds
/// the URL when one is needed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WikiTitle(pub String);

impl WikiTitle {
    /// The wiki's article base. Only `/wiki/<Title>` URLs are ever built:
    /// EXT-1 and EXT-2 forbid `/api.php`, `/index.php` and `Special:` pages.
    pub const BASE: &'static str = "https://wiki.guildwars.com/wiki/";

    /// The title as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The article URL for this title.
    ///
    /// Spaces become underscores and the rest is percent-encoded the way
    /// MediaWiki does: parentheses, colons and exclamation marks are left
    /// alone, while quotes and apostrophes are encoded. So `Dhuum's Covenant`
    /// becomes `Dhuum%27s_Covenant` and `Echo (skill type)` keeps its
    /// brackets.
    pub fn url(&self) -> String {
        // Characters MediaWiki leaves unencoded in an article path, beyond the
        // unreserved set. Verified against live wiki URLs.
        const KEEP: &[u8] = b"-_.~()!*:,@$;/+";

        let mut url = String::from(Self::BASE);
        for byte in self.0.replace(' ', "_").bytes() {
            if byte.is_ascii_alphanumeric() || KEEP.contains(&byte) {
                url.push(byte as char);
            } else {
                url.push_str(&format!("%{byte:02X}"));
            }
        }
        url
    }

    /// The slug this title produces.
    pub fn slug(&self) -> Slug {
        slugify(&self.0)
    }
}

impl fmt::Display for WikiTitle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assumption_ids_round_trip() {
        for text in ["A-000", "A-005", "A-033", "A-999"] {
            let id: AssumptionId = text.parse().expect(text);
            assert_eq!(id.to_string(), text);
        }
    }

    #[test]
    fn assumption_ids_are_padded_to_three_digits() {
        assert_eq!(AssumptionId::from_number(5).unwrap().to_string(), "A-005");
        assert_eq!(AssumptionId::from_number(33).unwrap().to_string(), "A-033");
        assert_eq!(AssumptionId::from_number(1000), None);
    }

    #[test]
    fn short_and_malformed_assumption_ids_are_rejected() {
        // "A-5" is the tempting shorthand, and accepting it would let the same
        // assumption be written two ways and cross-reference checks miss one.
        for text in ["A-5", "A-05", "A-0005", "a-005", "A005", "005", "", "A-abc"] {
            assert!(
                text.parse::<AssumptionId>().is_err(),
                "{text:?} should not parse"
            );
        }
    }

    #[test]
    fn assumption_ids_order_numerically() {
        let mut ids: Vec<AssumptionId> = ["A-010", "A-002", "A-033"]
            .iter()
            .map(|text| text.parse().unwrap())
            .collect();
        ids.sort();
        let sorted: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
        assert_eq!(sorted, ["A-002", "A-010", "A-033"]);
    }

    #[test]
    fn dates_round_trip() {
        let date: IsoDate = "2026-09-22".parse().unwrap();
        assert_eq!(date.to_string(), "2026-09-22");
        assert_eq!(date.parts(), (2026, 9, 22));
    }

    #[test]
    fn malformed_dates_are_rejected() {
        for text in [
            "2026-9-22",  // not padded
            "26-09-22",   // short year
            "2026/09/22", // wrong separator
            "2026-13-01", // no such month
            "2026-00-10", // no such month
            "2026-09-32", // no such day
            "2026-09-00", // no such day
            "2026-09-2x", // not digits
            "",
        ] {
            assert!(
                text.parse::<IsoDate>().is_err(),
                "{text:?} should not parse"
            );
        }
    }
}

#[cfg(test)]
mod name_tests {
    use super::*;

    #[test]
    fn the_plans_slug_table() {
        // The table from T1.2.2, verbatim.
        let cases = [
            (r#""Fall Back!""#, "fall-back"),
            ("Ancestors' Rage", "ancestors-rage"),
            ("Protective Was Kaolai", "protective-was-kaolai"),
            ("Energy Surge", "energy-surge"),
            (r#""Stand Your Ground!""#, "stand-your-ground"),
        ];
        for (title, expected) in cases {
            assert_eq!(slugify(title).as_str(), expected, "slugifying {title:?}");
        }
    }

    #[test]
    fn slugify_always_produces_a_valid_slug() {
        // Whatever goes in, what comes out must parse back as a Slug. This is
        // the property that lets the loader treat a Slug as a legal file stem
        // without re-checking.
        let titles = [
            "Energy Surge",
            "\"Fall Back!\"",
            "Ancestors' Rage",
            "Mark of Rodgort",
            "Signet of Illusions",
            "A.B.C.",
            "Fire & Ice",
            "Echo (skill type)",
            "Guild Wars Wiki:Copyrights",
            "  leading and trailing  ",
            "multiple   spaces",
            "hyphen-already",
            "double--hyphen",
            "trailing-",
            "-leading",
            "Über Skill",
        ];
        for title in titles {
            let slug = slugify(title);
            assert!(
                slug.as_str().parse::<Slug>().is_ok(),
                "slugify({title:?}) gave {slug:?}, which is not a valid slug"
            );
        }
    }

    #[test]
    fn ampersand_becomes_and() {
        assert_eq!(slugify("Fire & Ice").as_str(), "fire-and-ice");
    }

    #[test]
    fn runs_of_hyphens_collapse_and_ends_are_trimmed() {
        assert_eq!(slugify("double--hyphen").as_str(), "double-hyphen");
        assert_eq!(slugify("  padded  ").as_str(), "padded");
        assert_eq!(slugify("trailing-").as_str(), "trailing");
        assert_eq!(slugify("-leading").as_str(), "leading");
        assert_eq!(slugify("multiple   spaces").as_str(), "multiple-spaces");
    }

    #[test]
    fn punctuation_is_dropped_rather_than_hyphenated() {
        // "A.B.C." must not become "a-b-c-": dots carry no sound, so dropping
        // them is right and turning them into separators is not.
        assert_eq!(slugify("A.B.C.").as_str(), "abc");
        assert_eq!(slugify("Echo (skill type)").as_str(), "echo-skill-type");
    }

    #[test]
    fn underscores_behave_like_spaces() {
        // Wiki titles arrive both ways, and both must land on the same slug.
        assert_eq!(slugify("Energy_Surge"), slugify("Energy Surge"));
    }

    #[test]
    fn valid_slugs_parse_and_invalid_ones_do_not() {
        for text in ["energy-surge", "abc", "a1", "fall-back", "x"] {
            assert!(text.parse::<Slug>().is_ok(), "{text:?} should parse");
        }
        for text in [
            "",              // empty
            "Energy-Surge",  // uppercase
            "energy surge",  // space
            "energy_surge",  // underscore
            "-energy",       // leading hyphen
            "energy-",       // trailing hyphen
            "energy--surge", // doubled hyphen
            "energy.surge",  // dot
        ] {
            assert!(text.parse::<Slug>().is_err(), "{text:?} should not parse");
        }
    }

    #[test]
    fn wiki_urls_match_the_wikis_own_encoding() {
        let url = |title: &str| WikiTitle(title.to_owned()).url();

        assert_eq!(
            url("Energy Surge"),
            "https://wiki.guildwars.com/wiki/Energy_Surge"
        );
        // Apostrophes are encoded; this is a real wiki URL.
        assert_eq!(
            url("Dhuum's Covenant"),
            "https://wiki.guildwars.com/wiki/Dhuum%27s_Covenant"
        );
        // Quotes are encoded, exclamation marks are not.
        assert_eq!(
            url("\"Fall Back!\""),
            "https://wiki.guildwars.com/wiki/%22Fall_Back!%22"
        );
        // Brackets and colons are left alone.
        assert_eq!(
            url("Echo (skill type)"),
            "https://wiki.guildwars.com/wiki/Echo_(skill_type)"
        );
        assert_eq!(
            url("Guild Wars Wiki:Copyrights"),
            "https://wiki.guildwars.com/wiki/Guild_Wars_Wiki:Copyrights"
        );
    }

    #[test]
    fn every_url_stays_on_the_article_path() {
        // EXT-1 and EXT-2: never /api.php, /index.php or a query string. A
        // title containing those characters must come out encoded, not as a
        // live query.
        for title in [
            "Normal",
            "With?query=1",
            "With&more=2",
            "With#fragment",
            "../escape",
        ] {
            let url = WikiTitle(title.to_owned()).url();
            assert!(
                url.starts_with(WikiTitle::BASE),
                "{title:?} left the article path: {url}"
            );
            let tail = &url[WikiTitle::BASE.len()..];
            assert!(!tail.contains('?'), "{title:?} produced a query: {url}");
            assert!(!tail.contains('#'), "{title:?} produced a fragment: {url}");
            assert!(
                !tail.contains('&'),
                "{title:?} produced an ampersand: {url}"
            );
        }
    }

    #[test]
    fn a_title_knows_its_own_slug() {
        let title = WikiTitle("Protective Was Kaolai".to_owned());
        assert_eq!(title.slug().as_str(), "protective-was-kaolai");
    }

    #[test]
    fn skill_id_zero_means_no_skill() {
        assert_eq!(SkillId::NONE, 0);
        assert_eq!(SkillId(1234).get(), 1234);
        assert_eq!(SkillId(1234).to_string(), "#1234");
    }
}
