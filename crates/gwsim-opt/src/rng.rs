//! The optimiser's random choices, from one seed (ENG-1).
//!
//! A search must give the same frontier for the same seed, whatever the
//! thread count, so every random choice comes from this stream, drawn on the
//! search's own thread in a fixed order. Simulation randomness is separate:
//! it comes from the engine's per-run streams.

use gwsim_engine::rng::splitmix64;

/// A small deterministic generator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptRng {
    state: u64,
}

impl OptRng {
    pub fn new(seed: u64) -> OptRng {
        OptRng { state: seed }
    }

    /// A child stream for an independent purpose, so adding draws in one
    /// place does not shift every other.
    pub fn fork(&mut self, salt: u64) -> OptRng {
        OptRng::new(self.next_u64() ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15))
    }

    pub fn next_u64(&mut self) -> u64 {
        splitmix64(&mut self.state)
    }

    /// Uniform in `0..n` (0 when `n` is 0).
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.next_u64() % n as u64) as usize
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// True with probability `p`.
    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    /// One element, or `None` from an empty slice.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            None
        } else {
            Some(&items[self.below(items.len())])
        }
    }

    /// An index drawn in proportion to non-negative weights; uniform if they
    /// are all zero.
    pub fn weighted(&mut self, weights: &[f64]) -> usize {
        let total: f64 = weights.iter().filter(|w| **w > 0.0).sum();
        if total <= 0.0 {
            return self.below(weights.len());
        }
        let mut target = self.unit() * total;
        for (index, weight) in weights.iter().enumerate() {
            if *weight <= 0.0 {
                continue;
            }
            if target < *weight {
                return index;
            }
            target -= weight;
        }
        weights.len() - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stream_repeats_from_its_seed() {
        let mut a = OptRng::new(7);
        let mut b = OptRng::new(7);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn weighted_draws_follow_their_weights() {
        let mut rng = OptRng::new(1);
        let mut counts = [0usize; 3];
        for _ in 0..30_000 {
            counts[rng.weighted(&[1.0, 2.0, 0.0])] += 1;
        }
        assert_eq!(counts[2], 0);
        let ratio = counts[1] as f64 / counts[0] as f64;
        assert!((ratio - 2.0).abs() < 0.1, "{counts:?}");
    }
}
