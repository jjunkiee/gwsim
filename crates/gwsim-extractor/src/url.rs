//! The only URLs the extractor can ask for (T2.2.2, EXT-1, EXT-2).
//!
//! Two layers, deliberately redundant:
//!
//! 1. **Construction.** [`WikiUrl`] has a private field, so the only ways to
//!    get one are [`WikiUrl::article`], [`WikiUrl::robots`] and
//!    [`WikiUrl::from_redirect`]. There is no constructor that takes an
//!    arbitrary string.
//! 2. **Request time.** [`guard`] re-checks the finished URL for anything the
//!    wiki forbids — `/api.php`, `/index.php`, `Special:` in any case, a query
//!    string, another host — immediately before the transport is called.
//!
//! The second layer exists because the first can be undermined by a title:
//! `WikiUrl::article("Special:Random")` is a well-formed article URL for a
//! forbidden page. The guard catches what the type cannot.

use std::fmt;

use gwsim_data::WikiTitle;

/// The wiki's host. No other host is ever contacted.
pub const HOST: &str = "wiki.guildwars.com";

/// The one URL outside `/wiki/` the extractor may fetch.
pub const ROBOTS_URL: &str = "https://wiki.guildwars.com/robots.txt";

/// A URL the extractor is allowed to build.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WikiUrl(String);

impl WikiUrl {
    /// The article URL for a title: `https://wiki.guildwars.com/wiki/<Title>`.
    pub fn article(title: &WikiTitle) -> WikiUrl {
        WikiUrl(title.url())
    }

    /// `robots.txt`, the single allowed non-article URL.
    pub fn robots() -> WikiUrl {
        WikiUrl(ROBOTS_URL.to_owned())
    }

    /// The target of an HTTP redirect, if it is an allowed article URL.
    ///
    /// Accepts an absolute URL on the wiki's host or a path beginning
    /// `/wiki/`, and refuses anything that would not pass [`guard`].
    pub fn from_redirect(location: &str) -> Result<WikiUrl, Refusal> {
        let absolute = if location.starts_with("/wiki/") {
            format!("https://{HOST}{location}")
        } else if let Some(rest) = location.strip_prefix("//") {
            format!("https://{rest}")
        } else {
            location.to_owned()
        };
        guard(&absolute)?;
        if !absolute.starts_with(WikiTitle::BASE) {
            return Err(Refusal::NotAnArticle(absolute));
        }
        Ok(WikiUrl(absolute))
    }

    /// The URL as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The path part, from the first `/` after the host.
    pub fn path(&self) -> &str {
        let rest = self.0.strip_prefix("https://").unwrap_or(&self.0);
        match rest.find('/') {
            Some(index) => &rest[index..],
            None => "/",
        }
    }

    /// The title an article URL names, decoded, if it is an article URL.
    pub fn title(&self) -> Option<WikiTitle> {
        let encoded = self.0.strip_prefix(WikiTitle::BASE)?;
        Some(WikiTitle(percent_decode(encoded).replace('_', " ")))
    }
}

impl fmt::Display for WikiUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why a URL was refused before any request was made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Not `https://wiki.guildwars.com/…`.
    WrongHost(String),
    /// One of the endpoints robots.txt disallows for every agent (C1).
    ForbiddenEndpoint(String),
    /// A `Special:` page, in any spelling or case.
    SpecialPage(String),
    /// A query string, which only the disallowed endpoints take.
    QueryString(String),
    /// Not an article, where only an article will do.
    NotAnArticle(String),
    /// Disallowed by the session's robots.txt.
    Robots(String),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::WrongHost(url) => write!(f, "{url}: not on {HOST}"),
            Refusal::ForbiddenEndpoint(url) => {
                write!(
                    f,
                    "{url}: /api.php, /index.php and /load.php are never requested"
                )
            }
            Refusal::SpecialPage(url) => write!(f, "{url}: Special: pages are never requested"),
            Refusal::QueryString(url) => {
                write!(f, "{url}: URLs with a query string are never requested")
            }
            Refusal::NotAnArticle(url) => write!(f, "{url}: not a /wiki/ article URL"),
            Refusal::Robots(url) => write!(f, "{url}: disallowed by robots.txt"),
        }
    }
}

impl std::error::Error for Refusal {}

/// The request-time check every URL passes before the transport sees it.
pub fn guard(url: &str) -> Result<(), Refusal> {
    let owned = || url.to_owned();
    let Some(rest) = url.strip_prefix("https://") else {
        return Err(Refusal::WrongHost(owned()));
    };
    let (host, path) = match rest.find('/') {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, "/"),
    };
    if !host.eq_ignore_ascii_case(HOST) {
        return Err(Refusal::WrongHost(owned()));
    }
    if url.contains('?') {
        return Err(Refusal::QueryString(owned()));
    }
    let lower = percent_decode(path).to_lowercase();
    for endpoint in ["/api.php", "/index.php", "/load.php"] {
        if lower.contains(endpoint) {
            return Err(Refusal::ForbiddenEndpoint(owned()));
        }
    }
    // Every localised spelling robots.txt lists, checked on the decoded path
    // so `Special%3ARandom` cannot slip past.
    for special in ["special:", "spezial:", "spécial:", "especial:"] {
        if lower.contains(special) {
            return Err(Refusal::SpecialPage(owned()));
        }
    }
    Ok(())
}

/// Decodes `%XX` sequences. Invalid sequences are kept as written.
pub fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let Some(byte) = std::str::from_utf8(&bytes[index + 1..index + 3])
                .ok()
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
        {
            out.push(byte);
            index += 3;
            continue;
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn title(text: &str) -> WikiTitle {
        WikiTitle(text.to_owned())
    }

    #[test]
    fn articles_are_built_under_wiki() {
        let url = WikiUrl::article(&title("Energy Surge"));
        assert_eq!(url.as_str(), "https://wiki.guildwars.com/wiki/Energy_Surge");
        assert_eq!(url.path(), "/wiki/Energy_Surge");
        assert!(guard(url.as_str()).is_ok());
    }

    #[test]
    fn a_question_mark_in_a_title_is_encoded_not_a_query() {
        let url = WikiUrl::article(&title("Why?"));
        assert!(!url.as_str().contains('?'));
        assert!(guard(url.as_str()).is_ok());
    }

    #[test]
    fn titles_decode_back_from_article_urls() {
        let url = WikiUrl::article(&title("\"Fall Back!\""));
        assert_eq!(url.title(), Some(title("\"Fall Back!\"")));
        let url = WikiUrl::article(&title("Enchanter's Conundrum"));
        assert_eq!(url.title(), Some(title("Enchanter's Conundrum")));
    }

    #[test]
    fn the_guard_refuses_every_forbidden_shape() {
        let refused = [
            "https://wiki.guildwars.com/api.php",
            "https://wiki.guildwars.com/index.php?title=Energy_Surge",
            "https://wiki.guildwars.com/wiki/Special:Random",
            "https://wiki.guildwars.com/wiki/special:random",
            "https://wiki.guildwars.com/wiki/Special%3ARandom",
            "https://wiki.guildwars.com/wiki/Spezial:Zufall",
            "https://wiki.guildwars.com/wiki/Energy_Surge?action=raw",
            "https://gwpvx.fandom.com/wiki/Build:Team",
            "http://wiki.guildwars.com/wiki/Energy_Surge",
            "https://wiki.guildwars.com/load.php",
        ];
        for url in refused {
            assert!(guard(url).is_err(), "{url} should be refused");
        }
    }

    #[test]
    fn a_special_page_title_builds_but_does_not_pass_the_guard() {
        // The reason the second layer exists.
        let url = WikiUrl::article(&title("Special:Random"));
        assert!(matches!(guard(url.as_str()), Err(Refusal::SpecialPage(_))));
    }

    #[test]
    fn redirect_targets_must_be_articles() {
        assert!(WikiUrl::from_redirect("/wiki/Energy_Surge").is_ok());
        assert!(WikiUrl::from_redirect("https://wiki.guildwars.com/wiki/Energy_Surge").is_ok());
        assert!(WikiUrl::from_redirect("/index.php?title=Energy_Surge").is_err());
        assert!(WikiUrl::from_redirect("https://example.com/wiki/Energy_Surge").is_err());
        assert!(WikiUrl::from_redirect("https://wiki.guildwars.com/images/x.png").is_err());
    }
}
