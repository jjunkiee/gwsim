//! Where the CLI gets its data from (T1.7.4).
//!
//! Three sources, in order: a directory the caller named, `./data` when it is
//! there, and otherwise the pack embedded in the binary. **Every command says
//! which one it used**, because a result computed against a developer's local
//! edits and one computed against the shipped data are different results, and
//! nothing else in the output would distinguish them.

use std::fmt;
use std::path::{Path, PathBuf};

use gwsim_data::dataset::DataSet;
use gwsim_data::error::DataErrors;
use gwsim_data::pack::{BASELINE, DataPack, PackVersion};
use gwsim_data::source::DirSource;

/// The data pack built into this binary.
const EMBEDDED: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/data.pack"));

/// The content hash of that pack, computed at build time.
pub const EMBEDDED_HASH: &str = env!("GWSIM_PACK_HASH");

/// Where a command's data came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataOrigin {
    /// A directory the caller named with `--data-dir`.
    Named(PathBuf),
    /// `./data`, found because it was there.
    WorkingDirectory(PathBuf),
    /// The pack built into this binary.
    Embedded,
}

impl fmt::Display for DataOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataOrigin::Named(path) => write!(f, "{} (--data-dir)", path.display()),
            DataOrigin::WorkingDirectory(path) => write!(f, "{}", path.display()),
            DataOrigin::Embedded => write!(f, "the data built into this binary"),
        }
    }
}

/// A loaded data set, and where it came from.
pub struct Loaded {
    pub data: DataSet,
    pub origin: DataOrigin,
    pub version: PackVersion,
}

/// Loads data from the first source that applies.
pub fn load(explicit: Option<&Path>) -> Result<Loaded, (DataOrigin, DataErrors)> {
    let origin = choose(explicit);

    match &origin {
        DataOrigin::Named(path) | DataOrigin::WorkingDirectory(path) => {
            let source = DirSource::new(path);
            match DataPack::from_source(&source) {
                Ok(pack) => Ok(Loaded {
                    data: pack.data,
                    version: pack.version,
                    origin,
                }),
                Err(problems) => Err((origin, problems)),
            }
        }
        DataOrigin::Embedded => match DataPack::from_bytes(EMBEDDED) {
            Ok(pack) => Ok(Loaded {
                data: pack.data,
                version: pack.version,
                origin,
            }),
            Err(problems) => Err((origin, problems)),
        },
    }
}

/// Decides which source to use, without reading it.
pub fn choose(explicit: Option<&Path>) -> DataOrigin {
    if let Some(path) = explicit {
        return DataOrigin::Named(path.to_path_buf());
    }
    let working = Path::new("data");
    if working.is_dir() {
        return DataOrigin::WorkingDirectory(working.to_path_buf());
    }
    DataOrigin::Embedded
}

/// The embedded pack's version, without loading the data.
pub fn embedded_version() -> PackVersion {
    PackVersion {
        content_hash: EMBEDDED_HASH.to_owned(),
        baseline: BASELINE.to_owned(),
        gwsim_version: env!("CARGO_PKG_VERSION").to_owned(),
    }
}

/// The version string `gwsim --version` prints.
///
/// Built by `build.rs` rather than at run time, because clap needs a
/// `&'static str`. It carries the data pack's short hash as well as the crate
/// version, because two builds of the same version can hold different data,
/// and a result is only reproducible against the data it came from (ENG-43).
pub const VERSION_STRING: &str = env!("GWSIM_VERSION_STRING");

/// The raw bytes of the embedded pack.
pub fn embedded_bytes() -> &'static [u8] {
    EMBEDDED
}

/// How large the embedded pack is, in bytes.
pub fn embedded_size() -> usize {
    EMBEDDED.len()
}
