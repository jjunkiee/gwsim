//! The local page cache and the crawl's resumable state (T2.2.5, EXT-5).
//!
//! Layout under the cache root (`.cache/wiki/` by default):
//!
//! - `<sanitised title>.html`: the raw body, exactly as served;
//! - `<sanitised title>.meta.json`: where and when it was fetched.
//!
//! The cache holds raw wiki HTML, so it is git-ignored and never committed
//! (§8.10). Nothing in it is copied into `data/` except numbers the parsers
//! extract (EXT-7).

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use gwsim_data::WikiTitle;
use serde::{Deserialize, Serialize};

/// What the cache knows about one page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageMeta {
    /// The URL that was requested.
    pub url: String,
    /// The title that was asked for.
    pub title: String,
    /// The title the wiki actually served, when a redirect was followed.
    #[serde(default)]
    pub canonical_title: Option<String>,
    /// When the body was last downloaded.
    pub fetched_at: String,
    /// When the page was last confirmed current, by a download or a 304.
    pub checked_at: String,
    /// The HTTP status of the last response.
    pub status: u16,
    /// Never sent by this wiki (T2.1 §1), kept in case that changes.
    #[serde(default)]
    pub etag: Option<String>,
    /// Sent back verbatim as `If-Modified-Since`.
    #[serde(default)]
    pub last_modified: Option<String>,
}

/// The page cache.
#[derive(Debug, Clone)]
pub struct PageCache {
    root: PathBuf,
}

impl PageCache {
    /// A cache rooted at a directory, which is created on first write.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        PageCache { root: root.into() }
    }

    /// The directory this cache lives in.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The file-name stem for a title.
    ///
    /// Safe as a Windows file name: spaces become underscores, and the
    /// characters Windows forbids (plus `%`, so the encoding is reversible)
    /// are percent-encoded.
    pub fn sanitise(title: &str) -> String {
        let mut out = String::with_capacity(title.len());
        for character in title.chars() {
            match character {
                ' ' => out.push('_'),
                ':' | '/' | '\\' | '"' | '?' | '*' | '<' | '>' | '|' | '%' => {
                    out.push_str(&format!("%{:02X}", character as u32));
                }
                other => out.push(other),
            }
        }
        out
    }

    fn html_path(&self, title: &str) -> PathBuf {
        self.root.join(format!("{}.html", Self::sanitise(title)))
    }

    fn meta_path(&self, title: &str) -> PathBuf {
        self.root
            .join(format!("{}.meta.json", Self::sanitise(title)))
    }

    /// What the cache knows about a title, if anything.
    pub fn meta(&self, title: &WikiTitle) -> Option<PageMeta> {
        let text = fs::read_to_string(self.meta_path(title.as_str())).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// A cached page's body, if one was downloaded.
    pub fn html(&self, title: &WikiTitle) -> Option<String> {
        fs::read_to_string(self.html_path(title.as_str())).ok()
    }

    /// Stores a downloaded page and its metadata.
    pub fn store(&self, meta: &PageMeta, body: &str) -> io::Result<()> {
        fs::create_dir_all(&self.root)?;
        fs::write(self.html_path(&meta.title), body)?;
        self.write_meta(meta)
    }

    /// Records a page that answered 404, removing any stale body.
    pub fn store_missing(&self, meta: &PageMeta) -> io::Result<()> {
        fs::create_dir_all(&self.root)?;
        let _ = fs::remove_file(self.html_path(&meta.title));
        self.write_meta(meta)
    }

    /// Marks a cached page as confirmed current, after a 304.
    pub fn touch(&self, title: &WikiTitle, checked_at: &str) -> io::Result<()> {
        if let Some(mut meta) = self.meta(title) {
            meta.checked_at = checked_at.to_owned();
            self.write_meta(&meta)?;
        }
        Ok(())
    }

    /// Removes a page from the cache. Tests use this to plant a removal.
    pub fn remove(&self, title: &WikiTitle) -> io::Result<()> {
        let _ = fs::remove_file(self.html_path(title.as_str()));
        let _ = fs::remove_file(self.meta_path(title.as_str()));
        Ok(())
    }

    fn write_meta(&self, meta: &PageMeta) -> io::Result<()> {
        let text = serde_json::to_string_pretty(meta).map_err(io::Error::other)?;
        fs::write(self.meta_path(&meta.title), text)
    }

    /// Every title the cache has metadata for, sorted.
    pub fn titles(&self) -> Vec<WikiTitle> {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut titles: Vec<WikiTitle> = entries
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".meta.json"))
            .filter_map(|entry| fs::read_to_string(entry.path()).ok())
            .filter_map(|text| serde_json::from_str::<PageMeta>(&text).ok())
            .map(|meta| WikiTitle(meta.title))
            .collect();
        titles.sort();
        titles.dedup();
        titles
    }
}

/// Which titles a crawl still has to fetch, and which it has done.
///
/// Saved after every page, so an interrupted crawl resumes where it stopped
/// rather than starting over (T2.2.5 step 4).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CrawlState {
    /// Titles still to fetch, in order.
    pub pending: Vec<String>,
    /// Titles already handled this crawl, whatever the outcome.
    pub completed: BTreeSet<String>,
}

impl CrawlState {
    /// Reads saved state, or an empty state if there is none.
    pub fn load(path: &Path) -> CrawlState {
        fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Writes the state.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        fs::write(path, text)
    }

    /// Adds titles to the end of the queue, skipping any already queued or
    /// done.
    pub fn enqueue(&mut self, titles: impl IntoIterator<Item = String>) {
        for title in titles {
            if !self.completed.contains(&title) && !self.pending.contains(&title) {
                self.pending.push(title);
            }
        }
    }

    /// Marks a title done and takes it off the queue.
    pub fn complete(&mut self, title: &str) {
        self.pending.retain(|pending| pending != title);
        self.completed.insert(title.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_become_safe_windows_file_names() {
        assert_eq!(PageCache::sanitise("Energy Surge"), "Energy_Surge");
        assert_eq!(
            PageCache::sanitise("Guild Wars Wiki:Game integration/Skills/1-500"),
            "Guild_Wars_Wiki%3AGame_integration%2FSkills%2F1-500"
        );
        assert_eq!(PageCache::sanitise("\"Fall Back!\""), "%22Fall_Back!%22");
        assert_eq!(PageCache::sanitise("Why?*"), "Why%3F%2A");
        assert_eq!(PageCache::sanitise("100%"), "100%25");
    }

    #[test]
    fn the_crawl_state_queues_each_title_once() {
        let mut state = CrawlState::default();
        state.enqueue(["A".to_owned(), "B".to_owned(), "A".to_owned()]);
        assert_eq!(state.pending, vec!["A", "B"]);
        state.complete("A");
        state.enqueue(["A".to_owned(), "C".to_owned()]);
        assert_eq!(state.pending, vec!["B", "C"]);
        assert!(state.completed.contains("A"));
    }
}
