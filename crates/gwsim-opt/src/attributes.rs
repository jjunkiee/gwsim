//! The attribute allocation heuristic (T5.1.5, §13.2).
//!
//! Each attribute is weighted by the bar skills that use it and how steeply
//! they scale: the sum, over those skills, of `|at15 − at0|` across their
//! scaled values (at least 1 per skill, so a skill whose scaling the wiki
//! does not tabulate still counts). Points are then bought greedily, one
//! rank at a time, by weight per point of the next rank's cost, within 200
//! points and rank 12. Whatever is left goes to the primary attribute, then
//! to the heaviest attributes.

use std::collections::BTreeMap;

use gwsim_data::build::Build;
use gwsim_data::core::Attribute;
use gwsim_data::dataset::DataSet;
use gwsim_data::derived::{MAX_POINTS, attribute_cost};

/// The highest rank points alone can buy.
pub const MAX_POINT_RANK: u8 = 12;

/// Each attribute's weight for a bar.
pub fn weights(build: &Build, data: &DataSet) -> BTreeMap<Attribute, f64> {
    let mut weights = BTreeMap::new();
    for id in build.skills.iter().flatten() {
        let Some(skill) = data.skill_by_id(*id) else {
            continue;
        };
        let Some(attribute) = skill.attribute else {
            continue;
        };
        let slope: f64 = skill
            .extracted
            .scaled
            .iter()
            .map(|v| f64::from((v.r15 - v.r0).abs()))
            .sum();
        *weights.entry(attribute).or_insert(0.0) += slope.max(1.0);
    }
    weights
}

/// The attributes a build may put points into.
fn available(build: &Build) -> Vec<Attribute> {
    let mut out: Vec<Attribute> = build.primary.attributes().to_vec();
    if let Some(secondary) = build.secondary {
        out.extend(
            secondary
                .attributes()
                .iter()
                .copied()
                .filter(|a| *a != secondary.primary_attribute()),
        );
    }
    out
}

fn step_cost(rank: u8) -> u16 {
    attribute_cost(rank + 1) - attribute_cost(rank)
}

/// Allocates a build's attribute points for its bar, replacing any it had.
pub fn allocate(build: &mut Build, data: &DataSet) {
    let weights = weights(build, data);
    let open = available(build);
    let mut ranks: BTreeMap<Attribute, u8> = BTreeMap::new();
    let mut spent: u16 = 0;

    // Weighted attributes first, by weight per point.
    loop {
        let best = open
            .iter()
            .filter_map(|a| {
                let weight = *weights.get(a)?;
                let rank = ranks.get(a).copied().unwrap_or(0);
                if rank >= MAX_POINT_RANK || spent + step_cost(rank) > MAX_POINTS {
                    return None;
                }
                Some((*a, weight / f64::from(step_cost(rank))))
            })
            .max_by(|x, y| x.1.total_cmp(&y.1).then(y.0.cmp(&x.0)));
        let Some((attribute, _)) = best else {
            break;
        };
        let rank = ranks.entry(attribute).or_insert(0);
        spent += step_cost(*rank);
        *rank += 1;
    }

    // Leftovers: the primary attribute, then anything still open, heaviest
    // first.
    let mut order = vec![build.primary.primary_attribute()];
    let mut rest: Vec<Attribute> = open.clone();
    rest.sort_by(|a, b| {
        let wa = weights.get(a).copied().unwrap_or(0.0);
        let wb = weights.get(b).copied().unwrap_or(0.0);
        wb.total_cmp(&wa).then(a.cmp(b))
    });
    order.extend(rest);
    for attribute in order {
        if !open.contains(&attribute) {
            continue;
        }
        loop {
            let rank = ranks.get(&attribute).copied().unwrap_or(0);
            if rank >= MAX_POINT_RANK || spent + step_cost(rank) > MAX_POINTS {
                break;
            }
            spent += step_cost(rank);
            ranks.insert(attribute, rank + 1);
        }
    }

    ranks.retain(|_, r| *r > 0);
    build.attribute_points = ranks;
}

/// Moves one rank from one attribute to another, then trims anything over
/// budget from the donor side (the "shift attribute points" mutation).
pub fn shift(build: &mut Build, from: Attribute, to: Attribute) -> bool {
    let open = available(build);
    if from == to || !open.contains(&to) {
        return false;
    }
    let Some(from_rank) = build.attribute_points.get(&from).copied() else {
        return false;
    };
    let to_rank = build.attribute_points.get(&to).copied().unwrap_or(0);
    if from_rank == 0 || to_rank >= MAX_POINT_RANK {
        return false;
    }
    build.attribute_points.insert(from, from_rank - 1);
    build.attribute_points.insert(to, to_rank + 1);
    while gwsim_data::derived::points_spent(build) > MAX_POINTS {
        let donor = build
            .attribute_points
            .iter()
            .filter(|(a, r)| **a != to && **r > 0)
            .max_by_key(|(a, r)| (**r, **a))
            .map(|(a, _)| *a);
        match donor {
            Some(a) => {
                let r = build.attribute_points[&a];
                build.attribute_points.insert(a, r - 1);
            }
            None => break,
        }
    }
    build.attribute_points.retain(|_, r| *r > 0);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use gwsim_data::derived::points_spent;
    use gwsim_data::source::DirSource;
    use std::path::Path;

    fn data() -> DataSet {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        DataSet::load(&DirSource::new(dir)).unwrap()
    }

    #[test]
    fn the_player_bar_gets_domination_highest() {
        // PvX gives the Energy Surge bar Domination 12, Fast Casting 10,
        // Inspiration 8 (§20.1). The heuristic weights Domination most (five
        // of its skills scale steeply on it) and gives Inspiration its two
        // skills' worth; Fast Casting, which no skill names, takes the
        // leftovers as the primary attribute.
        let data = data();
        let party = data.party(&"m1-mesmerway".parse().unwrap()).unwrap();
        let mut build = party.slots[0].build.clone();
        allocate(&mut build, &data);
        let dom = build.attribute_points[&Attribute::DominationMagic];
        assert_eq!(dom, 12, "{:?}", build.attribute_points);
        for (attribute, rank) in &build.attribute_points {
            assert!(*rank <= dom, "{attribute:?} above Domination");
        }
        assert!(points_spent(&build) <= MAX_POINTS);
        assert!(
            build
                .attribute_points
                .contains_key(&Attribute::InspirationMagic)
        );
    }

    #[test]
    fn a_shift_keeps_the_budget() {
        let data = data();
        let party = data.party(&"m1-mesmerway".parse().unwrap()).unwrap();
        let mut build = party.slots[0].build.clone();
        assert!(shift(
            &mut build,
            Attribute::InspirationMagic,
            Attribute::IllusionMagic
        ));
        assert!(points_spent(&build) <= MAX_POINTS);
        assert_eq!(build.attribute_points[&Attribute::IllusionMagic], 1);
    }
}
