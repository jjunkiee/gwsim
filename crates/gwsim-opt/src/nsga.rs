//! NSGA-II's selection machinery (T5.2.2; Deb et al. 2002).
//!
//! - **Fast non-dominated sorting** under constrained domination
//!   ([`crate::objectives::dominates`]): front 0 holds the candidates no
//!   other dominates, front 1 those dominated only by front 0, and so on.
//! - **Crowding distance** within a front: the sum over objectives of the
//!   normalised gap between each candidate's neighbours; the ends get
//!   infinity so the front's extremes survive.
//! - **Binary tournament** with the crowded comparison: lower front wins,
//!   then larger crowding distance.
//!
//! Every tie is broken by the canonical hash, so a run is reproducible.

use crate::objectives::{Score, dominates};
use crate::rng::OptRng;

/// Sorts candidates into fronts; each front lists indices into `scores`,
/// in hash order.
pub fn nondominated_sort(scores: &[&Score], hashes: &[u128]) -> Vec<Vec<usize>> {
    let n = scores.len();
    let mut dominated_by_count = vec![0usize; n];
    let mut dominates_list: Vec<Vec<usize>> = vec![Vec::new(); n];
    for i in 0..n {
        for j in 0..n {
            if i != j && dominates(scores[i], scores[j]) {
                dominates_list[i].push(j);
            } else if i != j && dominates(scores[j], scores[i]) {
                dominated_by_count[i] += 1;
            }
        }
    }
    let mut fronts = Vec::new();
    let mut current: Vec<usize> = (0..n).filter(|i| dominated_by_count[*i] == 0).collect();
    while !current.is_empty() {
        current.sort_by_key(|i| hashes[*i]);
        let mut next = Vec::new();
        for &i in &current {
            for &j in &dominates_list[i] {
                dominated_by_count[j] -= 1;
                if dominated_by_count[j] == 0 {
                    next.push(j);
                }
            }
        }
        fronts.push(current);
        current = next;
    }
    fronts
}

/// Crowding distances for one front, in the front's order.
pub fn crowding_distance(front: &[usize], scores: &[&Score], hashes: &[u128]) -> Vec<f64> {
    let m = front.len();
    let mut distance = vec![0.0; m];
    if m <= 2 {
        return vec![f64::INFINITY; m];
    }
    let objectives = scores[front[0]].objectives.len();
    // Infeasible fronts are ordered by violation, which counts as one more
    // objective so their spread is kept too.
    for k in 0..=objectives {
        let value = |i: usize| -> f64 {
            if k == objectives {
                scores[i].violation
            } else {
                scores[i].objectives[k]
            }
        };
        let mut order: Vec<usize> = (0..m).collect();
        order.sort_by(|a, b| {
            value(front[*a])
                .total_cmp(&value(front[*b]))
                .then(hashes[front[*a]].cmp(&hashes[front[*b]]))
        });
        let low = value(front[order[0]]);
        let high = value(front[order[m - 1]]);
        distance[order[0]] = f64::INFINITY;
        distance[order[m - 1]] = f64::INFINITY;
        let span = high - low;
        if span <= 0.0 || !span.is_finite() {
            continue;
        }
        for w in 1..m - 1 {
            let gap = value(front[order[w + 1]]) - value(front[order[w - 1]]);
            distance[order[w]] += gap / span;
        }
    }
    distance
}

/// Each candidate's front and crowding distance.
pub fn rank(scores: &[&Score], hashes: &[u128]) -> (Vec<usize>, Vec<f64>) {
    let fronts = nondominated_sort(scores, hashes);
    let mut front_of = vec![0; scores.len()];
    let mut crowd = vec![0.0; scores.len()];
    for (f, front) in fronts.iter().enumerate() {
        let distances = crowding_distance(front, scores, hashes);
        for (i, d) in front.iter().zip(distances) {
            front_of[*i] = f;
            crowd[*i] = d;
        }
    }
    (front_of, crowd)
}

/// The crowded comparison: whether `a` should be preferred to `b`.
pub fn crowded_better(a: usize, b: usize, front: &[usize], crowd: &[f64], hashes: &[u128]) -> bool {
    front[a]
        .cmp(&front[b])
        .then(crowd[b].total_cmp(&crowd[a]))
        .then(hashes[a].cmp(&hashes[b]))
        .is_lt()
}

/// Binary tournament selection.
pub fn tournament(front: &[usize], crowd: &[f64], hashes: &[u128], rng: &mut OptRng) -> usize {
    let n = front.len();
    let a = rng.below(n);
    let b = rng.below(n);
    if crowded_better(a, b, front, crowd, hashes) {
        a
    } else {
        b
    }
}

/// The best `keep` indices by the crowded comparison (environmental
/// selection).
pub fn select(front: &[usize], crowd: &[f64], hashes: &[u128], keep: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..front.len()).collect();
    order.sort_by(|a, b| {
        if crowded_better(*a, *b, front, crowd, hashes) {
            std::cmp::Ordering::Less
        } else if crowded_better(*b, *a, front, crowd, hashes) {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    });
    order.truncate(keep);
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(objectives: &[f64], violation: f64) -> Score {
        Score {
            situations: Vec::new(),
            objectives: objectives.to_vec(),
            goal: objectives[0],
            feasible: violation <= 0.0,
            violation,
        }
    }

    #[test]
    fn a_hand_made_population_sorts_into_its_fronts() {
        let s = [
            score(&[1.0, 5.0], 0.0), // 0: front 0
            score(&[2.0, 2.0], 0.0), // 1: front 0
            score(&[5.0, 1.0], 0.0), // 2: front 0
            score(&[3.0, 3.0], 0.0), // 3: front 1 (dominated by 1)
            score(&[4.0, 4.0], 0.0), // 4: front 2
            score(&[0.1, 0.1], 0.2), // 5: infeasible, behind every feasible
            score(&[0.1, 0.1], 0.5), // 6: more violation still
        ];
        let refs: Vec<&Score> = s.iter().collect();
        let hashes: Vec<u128> = (0..s.len() as u128).collect();
        let fronts = nondominated_sort(&refs, &hashes);
        assert_eq!(
            fronts,
            vec![vec![0, 1, 2], vec![3], vec![4], vec![5], vec![6]]
        );
    }

    #[test]
    fn the_extremes_of_a_front_are_kept() {
        let s = [
            score(&[1.0, 5.0], 0.0),
            score(&[2.0, 2.0], 0.0),
            score(&[2.1, 1.9], 0.0),
            score(&[5.0, 1.0], 0.0),
        ];
        let refs: Vec<&Score> = s.iter().collect();
        let hashes: Vec<u128> = (0..4).collect();
        let d = crowding_distance(&[0, 1, 2, 3], &refs, &hashes);
        assert!(d[0].is_infinite() && d[3].is_infinite());
        assert!(d[1] > 0.0 && d[1].is_finite());
        // The two middle points crowd each other, so selection keeps the
        // extremes first.
        let (front, crowd) = rank(&refs, &hashes);
        let kept = select(&front, &crowd, &hashes, 2);
        assert_eq!(kept, vec![0, 3]);
    }

    #[test]
    fn ties_break_by_hash() {
        let s = [score(&[1.0, 1.0], 0.0), score(&[1.0, 1.0], 0.0)];
        let refs: Vec<&Score> = s.iter().collect();
        let (front, crowd) = rank(&refs, &[9, 3]);
        assert!(crowded_better(1, 0, &front, &crowd, &[9, 3]));
    }
}
