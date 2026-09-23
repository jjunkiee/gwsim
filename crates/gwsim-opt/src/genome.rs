//! The genome: the free parts of a party, in a canonical form (T5.1.1,
//! T5.1.6, §13.1).
//!
//! A free slot's genome is a [`Build`] — primary (human slots only),
//! secondary, attribute points, eight skills, five armor pieces, headgear
//! attribute and weapon set, exactly the fields §13.1 lists — plus a
//! [`LockMask`] naming the parts inside it that must not change (OPT-3).
//! A locked slot carries its build unchanged and is never touched.

use gwsim_data::build::{Build, SlotKind};
use gwsim_data::party::PartyFile;
use serde::{Deserialize, Serialize};

/// Parts of a free slot the search must not change (OPT-3) [Proposed].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LockMask {
    /// Bar positions that stay as they are, one bit per position.
    pub skills: u8,
    /// Armor, headgear attribute and weapon set stay as they are.
    pub gear: bool,
    /// Attribute points stay as they are.
    pub attributes: bool,
    /// The secondary profession stays as it is.
    pub secondary: bool,
}

impl LockMask {
    /// Whether a bar position is locked.
    pub fn skill(&self, position: usize) -> bool {
        self.skills & (1 << position) != 0
    }
}

/// One free slot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreeGenome {
    pub build: Build,
    pub locks: LockMask,
    /// Human, hero or henchman: decides the primary and PvE-only rules.
    pub kind: SlotKind,
}

/// One party slot: fixed, or open to the search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SlotGenome {
    Locked(Build),
    Free(FreeGenome),
}

impl SlotGenome {
    pub fn build(&self) -> &Build {
        match self {
            SlotGenome::Locked(build) => build,
            SlotGenome::Free(free) => &free.build,
        }
    }
}

/// A whole party as the search sees it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartyGenome {
    pub slots: Vec<SlotGenome>,
}

impl PartyGenome {
    /// The genome of a party, with the named slots free (and their lock
    /// masks, or none). Every other slot is locked (OPT-3). Naming a slot
    /// frees it even when the party file marks it `locked`: the file's flag
    /// is the default for tools that vary a party on their own (RC1), and
    /// naming the slot is the user's explicit choice (F5.3).
    pub fn from_party(party: &PartyFile, free: &[(usize, LockMask)]) -> PartyGenome {
        let slots = party
            .slots
            .iter()
            .enumerate()
            .map(
                |(index, slot)| match free.iter().find(|(i, _)| *i == index) {
                    Some((_, locks)) => SlotGenome::Free(FreeGenome {
                        build: slot.build.clone(),
                        locks: *locks,
                        kind: slot.kind,
                    }),
                    _ => SlotGenome::Locked(slot.build.clone()),
                },
            )
            .collect();
        PartyGenome { slots }
    }

    /// Indices of the free slots.
    pub fn free_slots(&self) -> Vec<usize> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s, SlotGenome::Free(_)))
            .map(|(i, _)| i)
            .collect()
    }

    /// The party file this genome describes, on top of `base` (names,
    /// kinds, heroes and tactics come from there). A free slot whose bar
    /// changed drops its hand-written plan, so a plan is generated for the
    /// new bar (WP4.6).
    pub fn to_party(&self, base: &PartyFile) -> PartyFile {
        let mut party = base.clone();
        for (slot, genome) in party.slots.iter_mut().zip(&self.slots) {
            if let SlotGenome::Free(free) = genome {
                if free.build.skills != slot.build.skills {
                    slot.plan = None;
                }
                slot.build = free.build.clone();
            }
        }
        party
    }

    /// The canonical form: in each free slot, the unlocked skills sorted by
    /// id into the unlocked positions (bar order does not change play,
    /// because plans are generated), empty positions last.
    pub fn canonical(&self) -> PartyGenome {
        let mut out = self.clone();
        for slot in &mut out.slots {
            if let SlotGenome::Free(free) = slot {
                let open: Vec<usize> = (0..8).filter(|p| !free.locks.skill(*p)).collect();
                let mut skills: Vec<_> = open.iter().map(|p| free.build.skills[*p]).collect();
                skills.sort_by_key(|s| (s.is_none(), s.map(|id| id.get())));
                for (position, skill) in open.iter().zip(skills) {
                    free.build.skills[*position] = skill;
                }
            }
        }
        out
    }

    /// A stable 128-bit hash of the canonical form, which keys the
    /// evaluation cache and deduplication.
    pub fn hash(&self) -> u128 {
        let canonical = self.canonical();
        let builds: Vec<&Build> = canonical.slots.iter().map(SlotGenome::build).collect();
        let text = ron::to_string(&builds).unwrap_or_default();
        let digest = blake3::hash(text.as_bytes());
        let bytes: [u8; 16] = digest.as_bytes()[..16].try_into().unwrap_or([0; 16]);
        u128::from_le_bytes(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gwsim_data::core::Profession;
    use gwsim_data::ids::SkillId;
    use gwsim_data::party::PartySlot;

    fn party() -> PartyFile {
        let mut build = Build::new(Profession::Mesmer);
        build.skills[0] = Some(SkillId(39));
        build.skills[1] = Some(SkillId(75));
        PartyFile {
            name: "test".into(),
            notes: String::new(),
            sources: Vec::new(),
            benchmarks: Vec::new(),
            slots: vec![PartySlot {
                name: "player".into(),
                kind: SlotKind::Human,
                hero: None,
                build,
                locked: false,
                plan: None,
                disabled_skills: Vec::new(),
                published_code: None,
            }],
            tactics: None,
        }
    }

    #[test]
    fn a_genome_converts_to_a_party_and_back_unchanged() {
        let base = party();
        let genome = PartyGenome::from_party(&base, &[(0, LockMask::default())]);
        assert_eq!(genome.to_party(&base), base);
        assert_eq!(genome.free_slots(), vec![0]);
    }

    #[test]
    fn bar_order_does_not_change_the_hash() {
        let base = party();
        let genome = PartyGenome::from_party(&base, &[(0, LockMask::default())]);
        let mut swapped = genome.clone();
        if let SlotGenome::Free(free) = &mut swapped.slots[0] {
            free.build.skills.swap(0, 1);
            free.build.skills.swap(1, 5);
        }
        assert_ne!(swapped, genome);
        assert_eq!(swapped.hash(), genome.hash());
    }

    #[test]
    fn locked_positions_keep_their_place_in_the_canonical_form() {
        let base = party();
        let locks = LockMask {
            skills: 0b10,
            ..LockMask::default()
        };
        let genome = PartyGenome::from_party(&base, &[(0, locks)]);
        let canonical = genome.canonical();
        assert_eq!(canonical.slots[0].build().skills[1], Some(SkillId(75)));
    }

    #[test]
    fn only_named_slots_are_free() {
        let base = party();
        let genome = PartyGenome::from_party(&base, &[]);
        assert!(genome.free_slots().is_empty());
        let mut locked = party();
        locked.slots[0].locked = true;
        let genome = PartyGenome::from_party(&locked, &[(0, LockMask::default())]);
        assert_eq!(genome.free_slots(), vec![0], "naming a slot frees it");
    }
}
