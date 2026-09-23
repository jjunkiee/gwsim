//! A minimal `robots.txt` reader (T2.2.2, EXT-1).
//!
//! It implements what the wiki's file needs and no more:
//!
//! - groups introduced by one or more `User-agent` lines;
//! - `Allow` and `Disallow` rules, where the **longest** matching rule wins
//!   and `Allow` wins a tie;
//! - the `*` wildcard inside a rule and `$` as an end anchor, because the
//!   wiki's file writes `/wiki/Special:*` (T2.1 §1) and a prefix-only reader
//!   would treat that `*` as a literal character;
//! - `*` as the fallback group when no group names this agent.
//!
//! A robots.txt that cannot be fetched is never read as permission: the
//! client stops the session instead (T2.2.2 step 4).

/// The wiki's robots.txt as fetched on 2026-09-23 (T2.1 §1), for tests.
pub const WIKI_ROBOTS_TXT: &str =
    "# Salt source: salt://roles/wiki-manager/files/config/robots.txt.j2

User-agent: *
Disallow: /index.php
Disallow: /api.php
Disallow: /load.php
Disallow: /wiki/Special:*
Disallow: /wiki/Spezial:*
Disallow: /wiki/Spécial:*
Disallow: /wiki/Especial:*
";

/// The product token this crawler identifies itself by.
pub const AGENT_TOKEN: &str = "gwsim-extractor";

/// A parsed robots.txt.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Robots {
    groups: Vec<Group>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Group {
    agents: Vec<String>,
    rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Rule {
    allow: bool,
    pattern: String,
}

impl Robots {
    /// Parses a robots.txt. Unknown lines are ignored, as the format requires.
    pub fn parse(text: &str) -> Robots {
        let mut groups: Vec<Group> = Vec::new();
        // A run of User-agent lines opens one group; the first rule after
        // them closes the run.
        let mut collecting_agents = false;

        for raw in text.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            let Some((field, value)) = line.split_once(':') else {
                continue;
            };
            let field = field.trim().to_ascii_lowercase();
            let value = value.trim();

            match field.as_str() {
                "user-agent" => {
                    if !collecting_agents {
                        groups.push(Group::default());
                        collecting_agents = true;
                    }
                    if let Some(group) = groups.last_mut() {
                        group.agents.push(value.to_ascii_lowercase());
                    }
                }
                "allow" | "disallow" => {
                    collecting_agents = false;
                    // An empty Disallow means "allow everything" and adds no rule.
                    if value.is_empty() {
                        continue;
                    }
                    if let Some(group) = groups.last_mut() {
                        group.rules.push(Rule {
                            allow: field == "allow",
                            pattern: value.to_owned(),
                        });
                    }
                }
                _ => {}
            }
        }

        Robots { groups }
    }

    /// Whether a path may be fetched by this crawler.
    pub fn allows(&self, path: &str) -> bool {
        let Some(group) = self.group_for(AGENT_TOKEN) else {
            return true;
        };
        // The decoded form is checked too, so an encoded `Special%3A` is
        // judged the same as the plain spelling.
        let decoded = crate::url::percent_decode(path);
        let mut best: Option<&Rule> = None;
        for rule in &group.rules {
            if !(matches(&rule.pattern, path) || matches(&rule.pattern, &decoded)) {
                continue;
            }
            best = match best {
                None => Some(rule),
                Some(current) => {
                    let longer = rule.pattern.len() > current.pattern.len();
                    let tie_allow = rule.pattern.len() == current.pattern.len() && rule.allow;
                    if longer || tie_allow {
                        Some(rule)
                    } else {
                        Some(current)
                    }
                }
            };
        }
        best.is_none_or(|rule| rule.allow)
    }

    /// The group that applies to an agent: the first naming it, else `*`.
    fn group_for(&self, agent: &str) -> Option<&Group> {
        let agent = agent.to_ascii_lowercase();
        self.groups
            .iter()
            .find(|group| {
                group
                    .agents
                    .iter()
                    .any(|name| name != "*" && agent.contains(name.as_str()))
            })
            .or_else(|| {
                self.groups
                    .iter()
                    .find(|group| group.agents.iter().any(|name| name == "*"))
            })
    }
}

/// Whether a rule pattern matches the start of a path, with `*` and `$`.
fn matches(pattern: &str, path: &str) -> bool {
    let (pattern, anchored) = match pattern.strip_suffix('$') {
        Some(stripped) => (stripped, true),
        None => (pattern, false),
    };
    matches_from(pattern.as_bytes(), path.as_bytes(), anchored)
}

fn matches_from(pattern: &[u8], path: &[u8], anchored: bool) -> bool {
    match pattern.split_first() {
        None => !anchored || path.is_empty(),
        Some((b'*', rest)) => {
            (0..=path.len()).any(|skip| matches_from(rest, &path[skip..], anchored))
        }
        Some((byte, rest)) => match path.split_first() {
            Some((first, remaining)) if first == byte => matches_from(rest, remaining, anchored),
            _ => false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wiki() -> Robots {
        Robots::parse(WIKI_ROBOTS_TXT)
    }

    #[test]
    fn the_wiki_file_allows_articles() {
        let robots = wiki();
        assert!(robots.allows("/wiki/Energy_Surge"));
        assert!(robots.allows("/wiki/Guild_Wars_Wiki:Game_integration/Skills/1-500"));
        assert!(robots.allows("/wiki/Category:Mesmer_skills"));
        assert!(robots.allows("/robots.txt"));
    }

    #[test]
    fn the_wiki_file_disallows_its_endpoints_and_special_pages() {
        let robots = wiki();
        for path in [
            "/index.php",
            "/index.php?title=Energy_Surge",
            "/api.php?action=raw",
            "/load.php",
            "/wiki/Special:Random",
            "/wiki/Special%3ARandom",
            "/wiki/Spezial:Zufall",
        ] {
            assert!(!robots.allows(path), "{path} should be disallowed");
        }
    }

    #[test]
    fn the_longest_rule_wins_and_allow_wins_a_tie() {
        let robots = Robots::parse(
            "User-agent: *\nDisallow: /wiki/\nAllow: /wiki/Energy\nDisallow: /wiki/Energy_S\n",
        );
        assert!(!robots.allows("/wiki/Mistrust"));
        assert!(robots.allows("/wiki/Energy_Tap"));
        assert!(!robots.allows("/wiki/Energy_Surge"));

        let tie = Robots::parse("User-agent: *\nDisallow: /a\nAllow: /a\n");
        assert!(tie.allows("/a"));
    }

    #[test]
    fn a_named_group_beats_the_wildcard_group() {
        let robots = Robots::parse(
            "User-agent: *\nDisallow: /\n\nUser-agent: gwsim-extractor\nDisallow: /private\n",
        );
        assert!(robots.allows("/wiki/Energy_Surge"));
        assert!(!robots.allows("/private/x"));
    }

    #[test]
    fn several_agents_can_share_a_group() {
        let robots = Robots::parse("User-agent: a\nUser-agent: gwsim-extractor\nDisallow: /x\n");
        assert!(!robots.allows("/x"));
    }

    #[test]
    fn wildcards_and_anchors_match_as_specified() {
        assert!(matches("/wiki/Special:*", "/wiki/Special:Random"));
        assert!(matches("/*.php", "/index.php"));
        assert!(matches("/a$", "/a"));
        assert!(!matches("/a$", "/ab"));
        assert!(!matches("/wiki/Special:*", "/wiki/Energy_Surge"));
    }

    #[test]
    fn an_empty_file_allows_everything() {
        assert!(Robots::parse("").allows("/wiki/Anything"));
        assert!(Robots::parse("User-agent: *\nDisallow:\n").allows("/wiki/Anything"));
    }
}
