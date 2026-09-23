//! The optimiser's rune rules on top of `Build::check` (T5.1.3, OPT-1).
//!
//! `Build::check` already enforces rune legality (primary-profession
//! attribute runes only). What it cannot say is that a rune is *wasted*:
//! only the highest rune per attribute counts, and only the best Vigor, yet
//! **every** attribute rune's health penalty applies. A build carrying a
//! Minor and a Superior Domination rune pays two penalties for one bonus.
//! Such runes are legal but redundant, and repair removes them.

use gwsim_data::build::Build;
use gwsim_data::dataset::DataSet;
use gwsim_data::items::{Rune, RuneBonus, RuneKind};

/// Runes whose `non_stacking_key` ends with this stack with everything.
const STACKS: &str = ":stacks";

fn rune<'a>(data: &'a DataSet, build: &Build, piece: usize) -> Option<&'a Rune> {
    let slug = build.armor[piece].rune.as_ref()?;
    data.runes
        .as_ref()?
        .value
        .runes
        .iter()
        .find(|r| r.slug == *slug)
}

fn strength(rune: &Rune) -> u32 {
    match rune.bonus {
        Some(RuneBonus::AttributeRank(n)) => u32::from(n),
        Some(RuneBonus::MaxHealth(n)) | Some(RuneBonus::MaxEnergy(n)) => u32::from(n),
        Some(RuneBonus::ConditionReduction { percent }) => u32::from(percent),
        None => 0,
    }
}

/// Armor pieces whose rune adds nothing because a stronger (or equal,
/// earlier) rune with the same non-stacking key is worn elsewhere.
pub fn redundant_runes(build: &Build, data: &DataSet) -> Vec<usize> {
    let mut redundant = Vec::new();
    for piece in 0..build.armor.len() {
        let Some(this) = rune(data, build, piece) else {
            continue;
        };
        if this.non_stacking_key.ends_with(STACKS) {
            continue;
        }
        let beaten = (0..build.armor.len()).any(|other| {
            other != piece
                && rune(data, build, other).is_some_and(|that| {
                    that.non_stacking_key == this.non_stacking_key
                        && (strength(that) > strength(this)
                            || (strength(that) == strength(this) && other < piece))
                })
        });
        if beaten {
            redundant.push(piece);
        }
    }
    redundant
}

/// Attribute runes for an attribute the build has no points in: legal, and
/// they do add a rank, but they cost health for an attribute nothing uses.
pub fn attribute_rune_attribute(rune: &Rune) -> Option<gwsim_data::core::Attribute> {
    match &rune.kind {
        RuneKind::Attribute { attribute, .. } => Some(*attribute),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gwsim_data::core::Profession;
    use gwsim_data::source::DirSource;
    use std::path::Path;

    fn data() -> DataSet {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        DataSet::load(&DirSource::new(dir)).unwrap()
    }

    fn with_runes(runes: &[Option<&str>]) -> Build {
        let mut build = Build::new(Profession::Mesmer);
        for (piece, rune) in runes.iter().enumerate() {
            build.armor[piece].rune = rune.map(|s| s.parse().unwrap());
        }
        build
    }

    #[test]
    fn a_weaker_rune_for_the_same_attribute_is_redundant() {
        let data = data();
        let build = with_runes(&[
            Some("superior-domination-magic"),
            Some("minor-domination-magic"),
            None,
            None,
            None,
        ]);
        assert_eq!(redundant_runes(&build, &data), vec![1]);
    }

    #[test]
    fn two_equal_runes_keep_the_first() {
        let data = data();
        let build = with_runes(&[
            Some("minor-fast-casting"),
            Some("minor-fast-casting"),
            None,
            None,
            None,
        ]);
        assert_eq!(redundant_runes(&build, &data), vec![1]);
    }

    #[test]
    fn only_the_best_vigor_counts_but_vitae_stacks() {
        let data = data();
        let build = with_runes(&[
            Some("superior-vigor"),
            Some("major-vigor"),
            Some("vitae"),
            Some("vitae"),
            None,
        ]);
        assert_eq!(redundant_runes(&build, &data), vec![1]);
    }

    #[test]
    fn runes_for_different_attributes_are_all_useful() {
        let data = data();
        let build = with_runes(&[
            Some("superior-domination-magic"),
            Some("minor-fast-casting"),
            Some("minor-inspiration-magic"),
            Some("superior-vigor"),
            Some("vitae"),
        ]);
        assert!(redundant_runes(&build, &data).is_empty());
    }
}
