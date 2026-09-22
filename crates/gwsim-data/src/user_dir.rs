//! Where a person's own encounters, parties and results live (T1.7.5).
//!
//! **Nothing is created until something is saved.** A tool that makes folders
//! in `%APPDATA%` just for being run is a tool that litters, so the directory
//! is resolved eagerly and created lazily.

use std::path::{Path, PathBuf};

/// The environment variable that overrides the location.
pub const OVERRIDE_VAR: &str = "GWSIM_USER_DIR";

/// The folder name under the platform's data directory.
pub const FOLDER: &str = "gwsim";

/// The prefix a user-supplied entity's id carries.
///
/// User files are namespaced so they can never shadow a bundled id: a person
/// who writes their own `kournan-patrol` gets `user:kournan-patrol`, and the
/// bundled one keeps working.
pub const USER_PREFIX: &str = "user:";

/// The subfolders a user directory holds.
pub const SUBFOLDERS: [&str; 6] = [
    "profiles",
    "encounters",
    "situations",
    "situation_sets",
    "parties",
    "results",
];

/// Where a person's own files live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserDir {
    root: PathBuf,
    /// How the location was chosen, for `gwsim data info` to report.
    source: UserDirSource,
}

/// How a user directory's location was decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserDirSource {
    /// Passed on the command line.
    Argument,
    /// From `GWSIM_USER_DIR`.
    Environment,
    /// The platform's usual place.
    Platform,
}

impl UserDir {
    /// Resolves the location, preferring an explicit path, then the
    /// environment, then the platform default.
    ///
    /// Returns [`None`] only when there is no platform data directory and
    /// nothing was supplied, which should not happen on a supported system.
    pub fn resolve(explicit: Option<&Path>) -> Option<UserDir> {
        if let Some(path) = explicit {
            return Some(UserDir {
                root: path.to_path_buf(),
                source: UserDirSource::Argument,
            });
        }
        if let Some(path) = std::env::var_os(OVERRIDE_VAR)
            && !path.is_empty()
        {
            return Some(UserDir {
                root: PathBuf::from(path),
                source: UserDirSource::Environment,
            });
        }
        dirs::data_dir().map(|base| UserDir {
            root: base.join(FOLDER),
            source: UserDirSource::Platform,
        })
    }

    /// The directory itself.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// How its location was chosen.
    pub fn source(&self) -> UserDirSource {
        self.source
    }

    /// Whether it exists yet.
    pub fn exists(&self) -> bool {
        self.root.is_dir()
    }

    /// One of the subfolders.
    pub fn subfolder(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    /// Creates the directory and its subfolders.
    ///
    /// Call this only when something is actually being saved.
    pub fn create(&self) -> std::io::Result<()> {
        for name in SUBFOLDERS {
            std::fs::create_dir_all(self.subfolder(name))?;
        }
        Ok(())
    }

    /// The namespaced id a user file's slug becomes.
    pub fn namespaced(slug: &str) -> String {
        format!("{USER_PREFIX}{slug}")
    }

    /// Whether an id came from a user file.
    pub fn is_user_id(id: &str) -> bool {
        id.starts_with(USER_PREFIX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_explicit_path_wins() {
        let dir = UserDir::resolve(Some(Path::new("/tmp/somewhere"))).expect("should resolve");
        assert_eq!(dir.root(), Path::new("/tmp/somewhere"));
        assert_eq!(dir.source(), UserDirSource::Argument);
    }

    #[test]
    fn the_platform_default_ends_in_the_folder_name() {
        // Only meaningful where the platform has a data directory, which
        // Windows, macOS and Linux all do.
        if let Some(dir) = UserDir::resolve(None)
            && dir.source() == UserDirSource::Platform
        {
            assert!(dir.root().ends_with(FOLDER), "{:?}", dir.root());
        }
    }

    #[test]
    fn resolving_does_not_create_anything() {
        // The property that matters: running gwsim must not litter.
        let path = std::env::temp_dir().join("gwsim-user-dir-should-not-exist");
        let _ = std::fs::remove_dir_all(&path);

        let dir = UserDir::resolve(Some(&path)).expect("should resolve");
        assert!(!dir.exists(), "resolving must not create the directory");
    }

    #[test]
    fn creating_makes_every_subfolder() {
        let path = std::env::temp_dir().join("gwsim-user-dir-test");
        let _ = std::fs::remove_dir_all(&path);

        let dir = UserDir::resolve(Some(&path)).expect("should resolve");
        dir.create().expect("should create");

        assert!(dir.exists());
        for name in SUBFOLDERS {
            assert!(dir.subfolder(name).is_dir(), "{name} should exist");
        }

        std::fs::remove_dir_all(&path).ok();
    }

    #[test]
    fn user_ids_are_namespaced_so_they_cannot_shadow_bundled_ones() {
        let id = UserDir::namespaced("kournan-patrol");
        assert_eq!(id, "user:kournan-patrol");
        assert!(UserDir::is_user_id(&id));
        assert!(!UserDir::is_user_id("kournan-patrol"));
    }
}
