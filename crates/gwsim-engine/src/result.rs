//! What one run produced (T3.8.1, §7.3).
//!
//! Statistics are kept in vectors sized when the fight is set up, indexed by
//! party slot, fight skill and foe, rather than in maps, so recording them
//! costs an index and an add.

use gwsim_data::{AssumptionId, SkillId};
use serde::Serialize;

use crate::rng::RunSeed;
use crate::unit::{ENERGY_SCALE, Team, Unit};

/// How a fight ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Outcome {
    Win,
    Wipe,
    /// Counted as a loss (§12.3).
    Timeout,
}

/// Per party-slot totals.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct SlotStats {
    pub name: String,
    pub damage_dealt: i64,
    pub damage_taken: i64,
    pub healing_done: i64,
    pub healing_received: i64,
    pub overhealing: i64,
    pub energy_spent: i64,
    pub energy_gained: i64,
    pub skills_used: u32,
    pub interrupts: u32,
    pub deaths: u32,
    /// Energy at the end of the fight.
    pub energy_left: i32,
}

/// Per skill totals, indexed by fight skill.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct SkillStats {
    pub id: u16,
    pub uses: u32,
    pub damage: i64,
    pub healing: i64,
    pub mitigation: i64,
    pub interrupts: u32,
}

/// Everything recorded during one fight.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct RunStats {
    pub slots: Vec<SlotStats>,
    pub skills: Vec<SkillStats>,
    /// Time to kill per foe, in foe order.
    pub foe_ttk_ms: Vec<Option<u32>>,
    /// Energy per party slot, sampled every second.
    pub energy_samples: Vec<Vec<i16>>,
}

impl RunStats {
    /// Empty statistics sized for a fight's units and skills.
    pub fn new(units: &[Unit], skills: usize) -> Self {
        let slots = units
            .iter()
            .filter(|u| u.slot_index.is_some())
            .map(|u| SlotStats {
                name: u.name.clone(),
                ..SlotStats::default()
            })
            .collect::<Vec<_>>();
        let foes = units.iter().filter(|u| u.foe_index.is_some()).count();
        RunStats {
            energy_samples: vec![Vec::new(); slots.len()],
            slots,
            skills: (0..skills).map(|_| SkillStats::default()).collect(),
            foe_ttk_ms: vec![None; foes],
        }
    }

    /// Records every party slot's energy.
    pub fn sample_energy(&mut self, units: &[Unit]) {
        for unit in units {
            if let (Some(slot), Team::Party) = (unit.slot_index, unit.team)
                && let Some(samples) = self.energy_samples.get_mut(slot)
            {
                samples.push((unit.energy / ENERGY_SCALE) as i16);
            }
        }
    }
}

/// One run's result (§7.3).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RunResult {
    pub seed: u64,
    pub outcome: Outcome,
    /// Milliseconds from aggro to the end, for wins.
    pub clear_time_ms: Option<u32>,
    /// When the fight ended, from its start.
    pub ended_ms: u32,
    pub deaths: u32,
    /// Death penalty at the end, as a percentage (chains).
    pub dp_end: u8,
    /// Damage the party took in total.
    pub damage_taken: i64,
    /// Energy the party had left in total.
    pub energy_left: i32,
    pub stats: RunStats,
    pub assumptions_touched: Vec<AssumptionId>,
    pub draft_skills_used: Vec<SkillId>,
    /// The combat log, when one was kept.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log: Option<Vec<crate::log::LogEvent>>,
}

impl RunResult {
    /// Whether the fight was won.
    pub fn won(&self) -> bool {
        self.outcome == Outcome::Win
    }

    /// A stable hash of everything but the log, for determinism tests.
    pub fn digest(&self) -> u64 {
        let mut copy = self.clone();
        copy.log = None;
        let text = serde_json::to_string(&copy).unwrap_or_default();
        text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        })
    }

    /// The seed as the engine's type.
    pub fn run_seed(&self) -> RunSeed {
        RunSeed(self.seed)
    }
}
