//! Where a tree of data files is read from.

use std::io;
use std::path::{Path, PathBuf};

/// A tree of data files.
///
/// Two implementations exist: [`DirSource`] for a `data/` directory during
/// development, and [`MemSource`] for the pack embedded in a release binary
/// and for tests. The loader only ever sees this trait, so the same validation
/// runs against both.
///
/// **Paths are always relative and use forward slashes**, whatever the
/// platform. This is not cosmetic: WP1.7 hashes the sorted `(path, contents)`
/// pairs into the data pack version, and a hash that changed between Windows
/// and Linux would break reproducibility (ENG-43).
pub trait DataSource {
    /// Every file in the tree, as forward-slash relative paths.
    ///
    /// Order is not guaranteed; the loader sorts.
    fn list(&self) -> io::Result<Vec<String>>;

    /// One file's contents.
    fn read(&self, path: &str) -> io::Result<String>;

    /// A short description of where this data came from, for error messages
    /// and for `gwsim data info`.
    fn describe(&self) -> String;
}

/// A directory on disk, such as the repository's `data/`.
#[derive(Debug, Clone)]
pub struct DirSource {
    root: PathBuf,
}

impl DirSource {
    /// Reads from this directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        DirSource { root: root.into() }
    }

    /// The directory being read.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn walk(&self, dir: &Path, found: &mut Vec<String>) -> io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                self.walk(&path, found)?;
            } else if let Some(relative) = relative_slash_path(&self.root, &path) {
                found.push(relative);
            }
        }
        Ok(())
    }
}

/// Turns an absolute path into a forward-slash path relative to `root`.
fn relative_slash_path(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            std::path::Component::Normal(part) => parts.push(part.to_str()?.to_owned()),
            // A data tree has no need for `..` or absolute segments, and
            // silently accepting them would let a path escape the root.
            _ => return None,
        }
    }
    Some(parts.join("/"))
}

impl DataSource for DirSource {
    fn list(&self) -> io::Result<Vec<String>> {
        let mut found = Vec::new();
        self.walk(&self.root, &mut found)?;
        found.sort();
        Ok(found)
    }

    fn read(&self, path: &str) -> io::Result<String> {
        let mut full = self.root.clone();
        for part in path.split('/') {
            full.push(part);
        }
        std::fs::read_to_string(full)
    }

    fn describe(&self) -> String {
        self.root.display().to_string()
    }
}

/// A tree held in memory.
///
/// Used by the embedded data pack and by tests, which need to build a broken
/// tree without writing files to disk.
#[derive(Debug, Clone, Default)]
pub struct MemSource {
    files: Vec<(String, String)>,
    description: String,
}

impl MemSource {
    /// Builds a tree from `(path, contents)` pairs.
    pub fn new(files: impl IntoIterator<Item = (String, String)>) -> Self {
        let mut files: Vec<(String, String)> = files.into_iter().collect();
        files.sort_by(|left, right| left.0.cmp(&right.0));
        MemSource {
            files,
            description: "an in-memory data set".to_owned(),
        }
    }

    /// Sets what error messages call this source.
    pub fn described_as(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Adds or replaces one file.
    pub fn with(mut self, path: impl Into<String>, contents: impl Into<String>) -> Self {
        let path = path.into();
        let contents = contents.into();
        match self
            .files
            .iter_mut()
            .find(|(existing, _)| *existing == path)
        {
            Some(entry) => entry.1 = contents,
            None => self.files.push((path, contents)),
        }
        self.files.sort_by(|left, right| left.0.cmp(&right.0));
        self
    }

    /// Removes one file, if it is there.
    pub fn without(mut self, path: &str) -> Self {
        self.files.retain(|(existing, _)| existing != path);
        self
    }

    /// The `(path, contents)` pairs, sorted by path.
    pub fn files(&self) -> &[(String, String)] {
        &self.files
    }
}

impl DataSource for MemSource {
    fn list(&self) -> io::Result<Vec<String>> {
        Ok(self.files.iter().map(|(path, _)| path.clone()).collect())
    }

    fn read(&self, path: &str) -> io::Result<String> {
        self.files
            .iter()
            .find(|(existing, _)| existing == path)
            .map(|(_, contents)| contents.clone())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, path.to_owned()))
    }

    fn describe(&self) -> String {
        self.description.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_memory_source_lists_paths_in_order() {
        let source = MemSource::new([
            ("skills/mesmer/panic.ron".to_owned(), "b".to_owned()),
            ("core/professions.ron".to_owned(), "a".to_owned()),
        ]);
        assert_eq!(
            source.list().unwrap(),
            ["core/professions.ron", "skills/mesmer/panic.ron"]
        );
    }

    #[test]
    fn a_memory_source_reads_and_replaces() {
        let source = MemSource::default()
            .with("a.ron", "one")
            .with("b.ron", "two")
            .with("a.ron", "replaced");
        assert_eq!(source.read("a.ron").unwrap(), "replaced");
        assert_eq!(source.list().unwrap().len(), 2);

        let smaller = source.without("b.ron");
        assert_eq!(smaller.list().unwrap(), ["a.ron"]);
    }

    #[test]
    fn a_missing_file_is_a_not_found_error() {
        let source = MemSource::default();
        let error = source.read("nope.ron").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn a_directory_source_returns_forward_slash_paths() {
        // The property WP1.7's content hash depends on: the same tree must
        // produce the same path strings on Windows and on Linux.
        let root = std::env::temp_dir().join("gwsim-source-test");
        let nested = root.join("skills").join("mesmer");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("panic.ron"), "()").unwrap();
        std::fs::write(root.join("assumptions.ron"), "()").unwrap();

        let source = DirSource::new(&root);
        let listed = source.list().unwrap();
        assert_eq!(listed, ["assumptions.ron", "skills/mesmer/panic.ron"]);
        assert!(!listed.iter().any(|path| path.contains('\\')));

        // And a listed path reads back.
        assert_eq!(source.read("skills/mesmer/panic.ron").unwrap(), "()");

        std::fs::remove_dir_all(&root).ok();
    }
}
