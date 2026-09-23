//! Static game data for gwsim: the types every `data/` file deserialises
//! into, and the checks that say whether a tree of them is sound.
//!
//! The values themselves are not in this crate. They live in `data/`, are
//! derived from the Guild Wars Wiki, and carry a provenance block saying where
//! each came from and how far it has been checked. See `data/ATTRIBUTION.md`.

pub mod assumptions;
pub mod build;
pub mod checks;
pub mod checks_dsl;
pub mod core;
pub mod coverage;
pub mod dataset;
pub mod derived;
pub mod describe;
pub mod dsl;
pub mod error;
pub mod foe;
pub mod ids;
pub mod items;
pub mod pack;
pub mod provenance;
pub mod scenario;
pub mod skill;
pub mod skill_index;
pub mod source;
pub mod template;
pub mod units;
pub mod user_dir;

pub use build::Build;
pub use dataset::DataSet;
pub use error::{DataError, DataErrors};
pub use foe::Foe;
pub use ids::{AssumptionId, IsoDate, SkillId, Slug, WikiTitle, slugify};
pub use pack::{DataPack, PackVersion};
pub use provenance::{Provenance, ReviewStatus};
pub use skill::Skill;
pub use source::{DataSource, DirSource, MemSource};
pub use units::Seconds;
