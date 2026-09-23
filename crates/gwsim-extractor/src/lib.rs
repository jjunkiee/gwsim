//! The wiki extractor: crawler, seeder and change report (§9).
//!
//! A developer tool. It is **not shipped** with gwsim, because the data it
//! produces is committed to the repository instead (D8).
//!
//! The crate is split so that the rules it must never break are enforced by
//! construction rather than by care:
//!
//! - [`url`] can only build article URLs and `robots.txt` (EXT-2), and
//!   re-checks every URL at request time (EXT-1);
//! - [`robots`] obeys the wiki's `robots.txt` (EXT-1);
//! - [`client`] owns the only [`transport::Transport`], waits its turn
//!   through one rate limiter (EXT-3), and stops on 403 or 429 (EXT-6);
//! - [`cache`] keeps every page with its fetch time (EXT-5);
//! - the parsers read the cache and hand back numbers only, so wiki prose
//!   never reaches `data/` (EXT-7).
//!
//! Every network-facing piece takes a [`transport::Transport`] and a
//! [`clock::Clock`], so the tests drive the whole thing with scripted
//! responses and a clock that never really sleeps.

pub mod cache;
pub mod client;
pub mod clock;
pub mod crawl;
pub mod diff;
pub mod discovery;
pub mod foe;
pub mod html;
pub mod normalise;
pub mod robots;
pub mod seed;
pub mod skill;
pub mod transport;
pub mod update;
pub mod url;

/// The project's public home, named in the User-Agent (EXT-4).
///
/// A URL rather than a person: the wiki's operators can find the project,
/// its crawl rules and its issue tracker from it, and it carries no
/// personal details.
pub const REPO_URL: &str = "https://github.com/jjunkiee/gwsim";

/// The User-Agent every request carries (EXT-4).
pub fn user_agent() -> String {
    format!(
        "gwsim-extractor/{} (+{})",
        env!("CARGO_PKG_VERSION"),
        REPO_URL
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_user_agent_names_the_project_and_nobody_else() {
        let agent = user_agent();
        assert!(agent.starts_with("gwsim-extractor/"));
        assert!(agent.contains(REPO_URL));
        // EXT-4: no personal details, and an email address is the obvious one.
        assert!(!agent.contains('@'), "{agent}");
    }
}
