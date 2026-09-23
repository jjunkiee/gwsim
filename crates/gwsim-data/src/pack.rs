//! The data pack a shipped binary carries (§6.4, ENG-43).
//!
//! The pack is the **RON text itself**, concatenated with its paths, not a
//! binary serialisation. T1.7.1 measured a full-coverage tree at 36 ms to
//! parse in release, which is not a cost worth a second format for. What that
//! buys: the text in the binary is the text that was validated, going through
//! the same parser, so a serialisation bug is a class of failure that cannot
//! happen here.
//!
//! Validation still happens at build time — `build.rs` fails the build on a
//! broken tree — so a release binary cannot carry data that does not load.

use std::fmt;
use std::path::Path;

use crate::dataset::DataSet;
use crate::error::DataErrors;
use crate::source::{DataSource, DirSource, MemSource};

/// The format of the pack file itself.
///
/// Bumped only when the *container* changes, not when a schema does: the
/// contents are RON, and a schema change shows up as a different content
/// hash rather than a different format.
pub const FORMAT_VERSION: u8 = 1;

/// The date the bundled data describes.
///
/// Guild Wars is patched; this says which state of the game the numbers are
/// from, so a stale result is identifiable as stale (C4).
pub const BASELINE: &str = "2026-09-22";

/// A pack's identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackVersion {
    /// BLAKE3 over the sorted `(path, contents)` pairs.
    pub content_hash: String,
    /// The game state the data describes.
    pub baseline: String,
    /// The gwsim version that built it.
    pub gwsim_version: String,
}

impl PackVersion {
    /// The first twelve characters of the hash, which is what a report shows.
    pub fn short_hash(&self) -> &str {
        let end = self.content_hash.len().min(12);
        &self.content_hash[..end]
    }
}

impl fmt::Display for PackVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (baseline {}, gwsim {})",
            self.short_hash(),
            self.baseline,
            self.gwsim_version
        )
    }
}

/// A validated tree of data, and the identity of the bytes it came from.
#[derive(Debug, Clone)]
pub struct DataPack {
    pub version: PackVersion,
    pub data: DataSet,
}

impl DataPack {
    /// Loads and validates a tree, then packs it.
    pub fn from_source(source: &dyn DataSource) -> Result<DataPack, DataErrors> {
        let files = read_all(source)?;
        let data = DataSet::load(source)?;
        Ok(DataPack {
            version: PackVersion {
                content_hash: content_hash(&files),
                baseline: BASELINE.to_owned(),
                gwsim_version: env!("CARGO_PKG_VERSION").to_owned(),
            },
            data,
        })
    }

    /// Loads and validates a directory, then packs it.
    pub fn from_dir(root: impl AsRef<Path>) -> Result<DataPack, DataErrors> {
        DataPack::from_source(&DirSource::new(root.as_ref()))
    }

    /// Serialises a tree to pack bytes, without loading it.
    ///
    /// Used by `build.rs`, which validates first and then writes.
    pub fn bytes_from_source(source: &dyn DataSource) -> Result<Vec<u8>, DataErrors> {
        let files = read_all(source)?;
        Ok(to_bytes(&files))
    }

    /// Reads a pack from bytes and validates what it holds.
    pub fn from_bytes(bytes: &[u8]) -> Result<DataPack, DataErrors> {
        let files = parse_bytes(bytes)?;
        let source = MemSource::new(files).described_as("the embedded data pack");
        DataPack::from_source(&source)
    }
}

/// The files of a pack, as a source, without validating them.
///
/// For reading parts the [`DataSet`] does not hold, such as core data.
pub fn source_from_bytes(bytes: &[u8]) -> Result<MemSource, DataErrors> {
    Ok(MemSource::new(parse_bytes(bytes)?).described_as("the embedded data pack"))
}

/// Reads every file from a source, in sorted order.
fn read_all(source: &dyn DataSource) -> Result<Vec<(String, String)>, DataErrors> {
    let mut problems = DataErrors::default();

    let paths = match source.list() {
        Ok(paths) => paths,
        Err(error) => {
            problems.0.push(crate::error::DataError::new(
                source.describe(),
                crate::error::DataErrorKind::Io,
                format!("the data tree could not be listed: {error}"),
            ));
            return Err(problems);
        }
    };

    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        match source.read(&path) {
            Ok(contents) => files.push((path, contents)),
            Err(error) => problems.0.push(crate::error::DataError::new(
                path,
                crate::error::DataErrorKind::Io,
                format!("could not be read: {error}"),
            )),
        }
    }

    if problems.is_fatal() {
        return Err(problems);
    }
    // Sorting here is what makes the hash independent of listing order.
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(files)
}

/// BLAKE3 over the sorted `(path, contents)` pairs.
///
/// The path length is hashed before the path, and the same for contents, so
/// that no two different trees can produce the same byte stream by splitting
/// a name differently.
pub fn content_hash(files: &[(String, String)]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[FORMAT_VERSION]);
    for (path, contents) in files {
        hasher.update(&(path.len() as u64).to_le_bytes());
        hasher.update(path.as_bytes());
        hasher.update(&(contents.len() as u64).to_le_bytes());
        hasher.update(contents.as_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

/// Writes files to pack bytes.
fn to_bytes(files: &[(String, String)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(FORMAT_VERSION);
    out.extend_from_slice(&(files.len() as u32).to_le_bytes());
    for (path, contents) in files {
        out.extend_from_slice(&(path.len() as u32).to_le_bytes());
        out.extend_from_slice(path.as_bytes());
        out.extend_from_slice(&(contents.len() as u32).to_le_bytes());
        out.extend_from_slice(contents.as_bytes());
    }
    out
}

/// Reads files back from pack bytes.
fn parse_bytes(bytes: &[u8]) -> Result<Vec<(String, String)>, DataErrors> {
    let fail = |message: String| {
        DataErrors(vec![crate::error::DataError::new(
            "the embedded data pack",
            crate::error::DataErrorKind::Io,
            message,
        )])
    };

    let mut cursor = 0usize;
    let take = |cursor: &mut usize, count: usize| -> Option<&[u8]> {
        let end = cursor.checked_add(count)?;
        let slice = bytes.get(*cursor..end)?;
        *cursor = end;
        Some(slice)
    };

    let version = *bytes
        .first()
        .ok_or_else(|| fail("the pack is empty".to_owned()))?;
    cursor += 1;
    if version != FORMAT_VERSION {
        return Err(fail(format!(
            "the pack has format version {version}, but this build reads version \
             {FORMAT_VERSION}; rebuild it"
        )));
    }

    let count = take(&mut cursor, 4)
        .and_then(|slice| slice.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| fail("the pack ends before its file count".to_owned()))?;

    let mut files = Vec::with_capacity(count as usize);
    for index in 0..count {
        let mut read_string = |what: &str| -> Result<String, DataErrors> {
            let length = take(&mut cursor, 4)
                .and_then(|slice| slice.try_into().ok())
                .map(u32::from_le_bytes)
                .ok_or_else(|| {
                    fail(format!(
                        "the pack ends before entry {index}'s {what} length"
                    ))
                })?;
            let raw = take(&mut cursor, length as usize)
                .ok_or_else(|| fail(format!("the pack ends inside entry {index}'s {what}")))?;
            String::from_utf8(raw.to_vec())
                .map_err(|_| fail(format!("entry {index}'s {what} is not UTF-8")))
        };

        let path = read_string("path")?;
        let contents = read_string("contents")?;
        files.push((path, contents));
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files() -> Vec<(String, String)> {
        vec![
            ("a.ron".to_owned(), "(one)".to_owned()),
            ("b/c.ron".to_owned(), "(two)".to_owned()),
        ]
    }

    #[test]
    fn bytes_round_trip() {
        let original = files();
        let back = parse_bytes(&to_bytes(&original)).expect("should parse");
        assert_eq!(back, original);
    }

    #[test]
    fn an_empty_pack_round_trips() {
        let back = parse_bytes(&to_bytes(&[])).expect("should parse");
        assert!(back.is_empty());
    }

    #[test]
    fn the_hash_is_stable_between_runs() {
        assert_eq!(content_hash(&files()), content_hash(&files()));
    }

    #[test]
    fn the_hash_does_not_depend_on_order() {
        // read_all sorts, so two listings of the same tree hash the same.
        let mut reversed = files();
        reversed.reverse();
        reversed.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(content_hash(&files()), content_hash(&reversed));
    }

    #[test]
    fn different_contents_hash_differently() {
        let mut changed = files();
        changed[0].1 = "(changed)".to_owned();
        assert_ne!(content_hash(&files()), content_hash(&changed));
    }

    #[test]
    fn moving_a_boundary_between_path_and_contents_changes_the_hash() {
        // Without hashing lengths, ("ab", "c") and ("a", "bc") would produce
        // the same byte stream and so the same hash.
        let left = vec![("ab".to_owned(), "c".to_owned())];
        let right = vec![("a".to_owned(), "bc".to_owned())];
        assert_ne!(content_hash(&left), content_hash(&right));
    }

    #[test]
    fn a_truncated_pack_is_rejected_rather_than_read_as_garbage() {
        let bytes = to_bytes(&files());
        for cut in [1, 3, 6, 10, bytes.len() - 1] {
            assert!(
                parse_bytes(&bytes[..cut]).is_err(),
                "a pack cut to {cut} bytes should be rejected"
            );
        }
    }

    #[test]
    fn a_pack_from_a_future_format_says_so() {
        let mut bytes = to_bytes(&files());
        bytes[0] = FORMAT_VERSION + 1;
        let error = parse_bytes(&bytes).expect_err("should be rejected");
        assert!(error.0[0].message.contains("format version"), "{error}");
    }

    #[test]
    fn a_short_hash_is_twelve_characters() {
        let version = PackVersion {
            content_hash: content_hash(&files()),
            baseline: BASELINE.to_owned(),
            gwsim_version: "0.1.0".to_owned(),
        };
        assert_eq!(version.short_hash().len(), 12);
        assert!(version.to_string().contains(BASELINE));
    }
}
