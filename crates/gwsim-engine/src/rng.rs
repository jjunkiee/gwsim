//! Seeded, purpose-separated random streams (WP3.7, ENG-40 to ENG-42).
//!
//! Every random draw in a fight comes from a [`Stream`]: a ChaCha8 generator
//! keyed by the run seed, a [`Purpose`] and, usually, the unit making the
//! draw. Separate streams mean that changing one unit's build changes as
//! little of everyone else's randomness as possible (ENG-41): foe 1's hit
//! rolls against the player come from foe 1's hits stream, which nothing the
//! player does can advance.
//!
//! [`SeedList`] gives the optimiser's adaptive stages shared prefixes, so
//! candidates are always compared on the same seeds (ENG-42).

use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};

use crate::unit::UnitId;

/// One fight's seed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct RunSeed(pub u64);

/// What a stream is used for.
///
/// Numbered explicitly: the numbers feed stream derivation, so reordering
/// the variants must not change every result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Purpose {
    /// Miss, block and hit-location rolls.
    Hits = 1,
    /// Critical-hit rolls.
    Crits = 2,
    /// `Chance` nodes, weapon chance mods and random skill outcomes.
    SkillChance = 3,
    /// AI choices.
    Ai = 4,
    /// Spawn jitter.
    Spawn = 5,
    /// Consumable effects.
    Consumables = 6,
}

impl Purpose {
    /// Every purpose, in number order.
    pub const ALL: [Purpose; 6] = [
        Purpose::Hits,
        Purpose::Crits,
        Purpose::SkillChance,
        Purpose::Ai,
        Purpose::Spawn,
        Purpose::Consumables,
    ];
}

/// One SplitMix64 step: a fixed, well-mixed function from one u64 to the next.
pub fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A random stream.
#[derive(Debug, Clone)]
pub struct Stream {
    rng: ChaCha8Rng,
}

impl Stream {
    /// The stream for a seed, purpose and (optionally) unit.
    ///
    /// The three are mixed through SplitMix64 into a 32-byte ChaCha key, so
    /// streams for different purposes or units are unrelated (T3.7.2).
    pub fn derive(seed: RunSeed, purpose: Purpose, unit: Option<UnitId>) -> Stream {
        let mut state = seed.0
            ^ (purpose as u64).wrapping_mul(0xA24B_AED4_963E_E407)
            ^ unit
                .map(|u| (u64::from(u.0) + 1).wrapping_mul(0x9FB2_1C65_1E98_DF25))
                .unwrap_or(0);
        let mut key = [0u8; 32];
        for chunk in key.chunks_mut(8) {
            chunk.copy_from_slice(&splitmix64(&mut state).to_le_bytes());
        }
        Stream {
            rng: ChaCha8Rng::from_seed(key),
        }
    }

    /// A uniform draw in `[0, 1)`, with 53 bits of precision.
    pub fn unit_f64(&mut self) -> f64 {
        (self.rng.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// True with probability `p`, clamped to `[0, 1]`.
    pub fn chance(&mut self, p: f64) -> bool {
        if p <= 0.0 {
            return false;
        }
        if p >= 1.0 {
            // Still draw, so a certainty does not shift later draws.
            let _ = self.unit_f64();
            return true;
        }
        self.unit_f64() < p
    }

    /// A uniform integer in `low..=high`.
    pub fn range_inclusive(&mut self, low: i32, high: i32) -> i32 {
        if high <= low {
            let _ = self.unit_f64();
            return low;
        }
        let span = (high - low + 1) as f64;
        low + (self.unit_f64() * span).floor() as i32
    }

    /// Picks an index by weights. Weights need not sum to one.
    pub fn weighted(&mut self, weights: &[f64]) -> usize {
        let total: f64 = weights.iter().sum();
        let mut roll = self.unit_f64() * total;
        for (index, weight) in weights.iter().enumerate() {
            if roll < *weight {
                return index;
            }
            roll -= weight;
        }
        weights.len().saturating_sub(1)
    }

    /// A raw 64-bit draw.
    pub fn next_u64(&mut self) -> u64 {
        self.rng.next_u64()
    }
}

/// Every stream a fight needs: one per purpose per unit, plus one per
/// purpose with no unit.
#[derive(Debug, Clone)]
pub struct Streams {
    seed: RunSeed,
    per_unit: Vec<[Stream; 6]>,
    shared: [Stream; 6],
}

impl Streams {
    /// Streams for `units` units under a seed.
    ///
    /// Units are numbered by stable slot and foe order, so adding a skill to
    /// slot 3 does not renumber slot 5 (T3.7.2 step 3).
    pub fn new(seed: RunSeed, units: usize) -> Streams {
        let make =
            |unit: Option<UnitId>| Purpose::ALL.map(|purpose| Stream::derive(seed, purpose, unit));
        Streams {
            seed,
            per_unit: (0..units).map(|i| make(Some(UnitId(i as u16)))).collect(),
            shared: make(None),
        }
    }

    /// A unit's stream for a purpose. Units created mid-fight (minions,
    /// spirits) get their streams on first use.
    pub fn unit(&mut self, unit: UnitId, purpose: Purpose) -> &mut Stream {
        let index = usize::from(unit.0);
        while self.per_unit.len() <= index {
            let next = UnitId(self.per_unit.len() as u16);
            let seed = self.seed;
            self.per_unit
                .push(Purpose::ALL.map(|purpose| Stream::derive(seed, purpose, Some(next))));
        }
        &mut self.per_unit[index][purpose as usize - 1]
    }

    /// The stream for a purpose that belongs to no unit.
    pub fn shared(&mut self, purpose: Purpose) -> &mut Stream {
        &mut self.shared[purpose as usize - 1]
    }
}

/// A deterministic list of run seeds with stable prefixes (T3.7.4).
///
/// The first `k` seeds are the same whatever length is asked for, so 16
/// runs, then 64, then 256 share their first runs, which is what makes the
/// optimiser's comparisons paired (ENG-42).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedList {
    pub master: u64,
    seeds: Vec<RunSeed>,
}

impl SeedList {
    /// `n` seeds from a master seed.
    pub fn new(master: u64, n: usize) -> SeedList {
        let mut state = master;
        let seeds = (0..n).map(|_| RunSeed(splitmix64(&mut state))).collect();
        SeedList { master, seeds }
    }

    /// The seeds.
    pub fn seeds(&self) -> &[RunSeed] {
        &self.seeds
    }

    /// How many there are.
    pub fn len(&self) -> usize {
        self.seeds.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.seeds.is_empty()
    }

    /// The seed at an index, extending deterministically past the end.
    pub fn get(&self, index: usize) -> RunSeed {
        if let Some(seed) = self.seeds.get(index) {
            return *seed;
        }
        SeedList::new(self.master, index + 1).seeds[index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stream_is_reproducible_from_its_seed() {
        let mut first = Stream::derive(RunSeed(42), Purpose::Hits, Some(UnitId(3)));
        let mut second = Stream::derive(RunSeed(42), Purpose::Hits, Some(UnitId(3)));
        for _ in 0..1000 {
            assert_eq!(first.next_u64(), second.next_u64());
        }
    }

    #[test]
    fn streams_for_different_purposes_or_units_do_not_overlap() {
        let draw = |purpose, unit| {
            let mut stream = Stream::derive(RunSeed(7), purpose, unit);
            (0..10_000)
                .map(|_| stream.next_u64())
                .collect::<std::collections::BTreeSet<_>>()
        };
        let hits = draw(Purpose::Hits, Some(UnitId(0)));
        let crits = draw(Purpose::Crits, Some(UnitId(0)));
        let other_unit = draw(Purpose::Hits, Some(UnitId(1)));
        let shared = draw(Purpose::Hits, None);
        assert!(hits.is_disjoint(&crits));
        assert!(hits.is_disjoint(&other_unit));
        assert!(hits.is_disjoint(&shared));
    }

    #[test]
    fn a_seed_list_keeps_its_prefix_whatever_its_length() {
        let short = SeedList::new(99, 16);
        let long = SeedList::new(99, 256);
        assert_eq!(short.seeds(), &long.seeds()[..16]);
        assert_eq!(short.get(100), long.seeds()[100]);
    }

    #[test]
    fn chance_draws_match_their_probability() {
        let mut stream = Stream::derive(RunSeed(1), Purpose::SkillChance, None);
        let hits = (0..100_000).filter(|_| stream.chance(0.25)).count();
        assert!((hits as f64 / 100_000.0 - 0.25).abs() < 0.01, "{hits}");
    }

    #[test]
    fn weighted_picks_follow_their_weights() {
        let mut stream = Stream::derive(RunSeed(1), Purpose::SkillChance, None);
        let mut counts = [0usize; 3];
        for _ in 0..100_000 {
            counts[stream.weighted(&[0.5, 0.3, 0.2])] += 1;
        }
        assert!((counts[0] as f64 / 100_000.0 - 0.5).abs() < 0.01);
        assert!((counts[2] as f64 / 100_000.0 - 0.2).abs() < 0.01);
    }
}
