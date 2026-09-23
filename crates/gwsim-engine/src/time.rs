//! Simulation time and the event queue (T3.1.2, DESIGN §10.2).
//!
//! Time is an integer number of milliseconds. Discrete things — an
//! activation ending, an effect expiring, a projectile landing — are events
//! in a priority queue ordered by `(time, priority class, insertion
//! sequence)`, so two events at the same millisecond always come out in the
//! same order. Continuous things (movement, regeneration, AI) advance on the
//! fixed tick in [`crate::sim`].

use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;
use std::fmt;

use crate::unit::UnitId;

/// The engine's fixed tick for continuous processes (T3.1.4).
///
/// A modelling resolution rather than a game fact, so it is a constant here
/// rather than an assumption: it decides how finely movement and
/// regeneration are sampled, not what the game does.
pub const TICK_MS: u32 = 50;

/// A moment in a fight, in milliseconds since the fight began.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct SimTime(pub u32);

impl SimTime {
    /// The start of a fight.
    pub const ZERO: SimTime = SimTime(0);

    /// Milliseconds since the start.
    pub fn ms(self) -> u32 {
        self.0
    }

    /// This time plus a duration.
    ///
    /// A fight longer than 49 days cannot happen, so overflow is a bug, and
    /// is asserted rather than wrapped.
    pub fn plus(self, ms: u32) -> SimTime {
        SimTime(
            self.0
                .checked_add(ms)
                .expect("simulation time overflowed, which no fight can reach"),
        )
    }

    /// Seconds, for reports.
    pub fn secs(self) -> f64 {
        f64::from(self.0) / 1000.0
    }
}

impl fmt::Display for SimTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:>8.3}s", self.secs())
    }
}

/// The order of simultaneous events.
///
/// Declared in firing order. The reasoning: an effect that expires at the
/// same moment a hit lands is gone before the hit (the game's effect
/// monitor updates before damage resolves); an impact lands before the
/// shooter's own activation completes; an activation completes before its
/// aftercast ends; and triggers and AI see the settled state last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PriorityClass {
    EffectExpiry,
    ProjectileImpact,
    AttackHit,
    ActivationEnd,
    AftercastEnd,
    RechargeEnd,
    Triggers,
}

/// What an event does when it fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// A unit's activation finishes, if the activation is still `generation`.
    ActivationEnd { unit: UnitId, generation: u32 },
    /// A unit's aftercast finishes.
    AftercastEnd { unit: UnitId, generation: u32 },
    /// An effect instance expires, if it still exists.
    EffectExpiry { unit: UnitId, effect: u32 },
    /// A projectile arrives.
    ProjectileImpact { projectile: u32 },
    /// An attack swing reaches the moment it hits.
    AttackHit { unit: UnitId, generation: u32 },
    /// A copied skill reverts to its original (Arcane Echo).
    SlotRevert {
        unit: UnitId,
        slot: u8,
        generation: u32,
    },
    /// A created creature's time is up: a spirit's duration ends.
    CreatureExpiry { unit: UnitId },
    /// Nothing but a marker, used by tests.
    Marker(u32),
}

impl EventKind {
    /// The class that orders this kind among simultaneous events.
    pub fn class(self) -> PriorityClass {
        match self {
            EventKind::EffectExpiry { .. } | EventKind::CreatureExpiry { .. } => {
                PriorityClass::EffectExpiry
            }
            EventKind::ProjectileImpact { .. } => PriorityClass::ProjectileImpact,
            EventKind::AttackHit { .. } => PriorityClass::AttackHit,
            EventKind::ActivationEnd { .. } => PriorityClass::ActivationEnd,
            EventKind::AftercastEnd { .. } => PriorityClass::AftercastEnd,
            EventKind::SlotRevert { .. } => PriorityClass::RechargeEnd,
            EventKind::Marker(_) => PriorityClass::Triggers,
        }
    }
}

/// A scheduled event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub at: SimTime,
    pub class: PriorityClass,
    /// Insertion order, so ties are broken the same way on every run.
    pub seq: u32,
    pub kind: EventKind,
}

impl Ord for Event {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.at, self.class, self.seq).cmp(&(other.at, other.class, other.seq))
    }
}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// The event queue.
///
/// Cancellation is by generation: whatever an event refers to carries a
/// generation number, and an event whose generation no longer matches is
/// skipped when it is popped. That is cheaper than removing it from the heap
/// and cannot miss one.
#[derive(Debug, Clone, Default)]
pub struct EventQueue {
    heap: BinaryHeap<Reverse<Event>>,
    next_seq: u32,
    /// The time of the last event popped, to catch scheduling in the past.
    popped_at: SimTime,
}

impl EventQueue {
    /// A queue with room for `capacity` events before it reallocates.
    pub fn with_capacity(capacity: usize) -> Self {
        EventQueue {
            heap: BinaryHeap::with_capacity(capacity),
            next_seq: 0,
            popped_at: SimTime::ZERO,
        }
    }

    /// Schedules an event.
    pub fn schedule(&mut self, at: SimTime, kind: EventKind) {
        debug_assert!(
            at >= self.popped_at,
            "an event was scheduled at {at:?}, before the current time {:?}",
            self.popped_at
        );
        let event = Event {
            at,
            class: kind.class(),
            seq: self.next_seq,
            kind,
        };
        self.next_seq = self.next_seq.wrapping_add(1);
        self.heap.push(Reverse(event));
    }

    /// The next event's time, if any.
    pub fn peek_time(&self) -> Option<SimTime> {
        self.heap.peek().map(|Reverse(event)| event.at)
    }

    /// Removes and returns the next event if it is due by `until`.
    pub fn pop_due(&mut self, until: SimTime) -> Option<Event> {
        if self.peek_time()? > until {
            return None;
        }
        let Reverse(event) = self.heap.pop()?;
        self.popped_at = event.at;
        Some(event)
    }

    /// How many events are waiting.
    pub fn len(&self) -> usize {
        self.heap.len()
    }

    /// Whether nothing is waiting.
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simultaneous_events_come_out_by_class_then_sequence() {
        let mut queue = EventQueue::with_capacity(8);
        let unit = UnitId(0);
        queue.schedule(SimTime(100), EventKind::Marker(1));
        queue.schedule(
            SimTime(100),
            EventKind::ActivationEnd {
                unit,
                generation: 0,
            },
        );
        queue.schedule(SimTime(100), EventKind::EffectExpiry { unit, effect: 0 });
        queue.schedule(SimTime(100), EventKind::Marker(2));
        queue.schedule(SimTime(50), EventKind::Marker(3));

        let order: Vec<EventKind> = std::iter::from_fn(|| queue.pop_due(SimTime(1000)))
            .map(|event| event.kind)
            .collect();
        assert_eq!(
            order,
            vec![
                EventKind::Marker(3),
                EventKind::EffectExpiry { unit, effect: 0 },
                EventKind::ActivationEnd {
                    unit,
                    generation: 0
                },
                EventKind::Marker(1),
                EventKind::Marker(2),
            ]
        );
    }

    #[test]
    fn nothing_is_popped_before_it_is_due() {
        let mut queue = EventQueue::with_capacity(2);
        queue.schedule(SimTime(200), EventKind::Marker(1));
        assert!(queue.pop_due(SimTime(199)).is_none());
        assert!(queue.pop_due(SimTime(200)).is_some());
        assert!(queue.is_empty());
    }

    #[test]
    #[should_panic(expected = "overflowed")]
    fn time_overflow_is_a_bug() {
        let _ = SimTime(u32::MAX).plus(1);
    }
}
