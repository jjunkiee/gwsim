//! Generating a situation's tactics plan from the party's builds (T4.7.2,
//! §11.6), and turning it into positions, modes and a pre-fight sequence.
//!
//! The rules follow the PvX tactics notes for the M1 party (§20.1):
//!
//! - **Formation:** healers, protectors and spirit-casters stand in the
//!   backline, everyone else in the midline behind the player. Backline
//!   heroes go on **Guard**, midline ones on **Fight**.
//! - **Pre-fight:** Shelter, then Union, then Displacement, then Armor of
//!   Unfeeling, when they are on a bar — preceded by Soul Twisting when the
//!   same hero carries it, since it makes the rituals affordable.
//! - **Called target:** a foe whose bar heals, first.
//! - **Spread:** heroes stand at least an area apart when any foe skill does
//!   area damage.

use gwsim_data::dataset::DataSet;
use gwsim_data::foe::{Foe, SkillRef};
use gwsim_data::party::PartyFile;
use gwsim_data::skill::RoleTag;
use gwsim_data::tactics::{
    FormationPoint, HeroModeName, PreCast, SlotMode, TacticsPlan, TargetRule,
};

use gwsim_data::build::SlotKind;

/// Formation depths behind the leader, in gwinches (a proposal; the PvX
/// notes name lines, not distances).
const MIDLINE: f32 = -100.0;
const BACKLINE: f32 = -350.0;
/// Spacing along a line, and when spreading against area damage (a little
/// more than the nearby band, 252).
const SPACING: f32 = 120.0;
const SPREAD_SPACING: f32 = 270.0;

/// The pre-fight rituals, in the order the PvX notes give.
const PRE_CAST: [&str; 4] = ["shelter", "union", "displacement", "armor-of-unfeeling"];

/// Generates the plan for a party against a set of foes.
pub fn generate(party: &PartyFile, data: &DataSet, foes: &[&Foe]) -> TacticsPlan {
    let roles_of = |slot: &gwsim_data::party::PartySlot| -> Vec<RoleTag> {
        slot.build
            .skills
            .iter()
            .flatten()
            .filter_map(|id| data.skill_by_id(*id))
            .filter_map(|s| s.encoding.as_ref())
            .flat_map(|e| e.roles.iter().copied())
            .collect()
    };
    let slug_of = |id: gwsim_data::SkillId| data.skill_slug_for_id(id).map(|s| s.to_string());
    let has = |slot: &gwsim_data::party::PartySlot, slug: &str| {
        slot.build
            .skills
            .iter()
            .flatten()
            .any(|id| slug_of(*id).as_deref() == Some(slug))
    };

    let foe_roles: Vec<RoleTag> = foes
        .iter()
        .flat_map(|f| f.skills.iter())
        .filter_map(|fs| match &fs.skill {
            SkillRef::Slug(slug) => data.skill(slug),
            SkillRef::Id(id) => data.skill_by_id(*id),
        })
        .filter_map(|s| s.encoding.as_ref())
        .flat_map(|e| e.roles.iter().copied())
        .collect();
    let spread = foe_roles.contains(&RoleTag::Aoe);

    let mut plan = TacticsPlan {
        spread_against_aoe: spread,
        ..TacticsPlan::default()
    };
    let spacing = if spread { SPREAD_SPACING } else { SPACING };
    let (mut mid, mut back) = (0usize, 0usize);
    for slot in &party.slots {
        if slot.kind == SlotKind::Human {
            plan.formation.push(FormationPoint {
                slot: slot.name.clone(),
                x: 0.0,
                y: 0.0,
            });
            continue;
        }
        let roles = roles_of(slot);
        let support = roles
            .iter()
            .filter(|r| matches!(r, RoleTag::Healing | RoleTag::Protection | RoleTag::Spirit))
            .count();
        let backline = support * 2 >= roles.len().max(1);
        let (line, depth, mode) = if backline {
            (&mut back, BACKLINE, HeroModeName::Guard)
        } else {
            (&mut mid, MIDLINE, HeroModeName::Fight)
        };
        // Alternate sides: 0, +1, -1, +2, -2, …
        let n = *line as f32;
        let side = if *line % 2 == 0 {
            -(n / 2.0)
        } else {
            n.div_euclid(2.0) + 1.0
        };
        *line += 1;
        plan.formation.push(FormationPoint {
            slot: slot.name.clone(),
            x: side * spacing,
            y: depth,
        });
        plan.hero_modes.push(SlotMode {
            slot: slot.name.clone(),
            mode,
        });
    }

    for ritual in PRE_CAST {
        for slot in party.slots.iter().filter(|s| has(s, ritual)) {
            let twisting = has(slot, "soul-twisting");
            let already = plan.pre_fight.iter().any(|p| {
                p.slot == slot.name && p.skill == SkillRef::Slug("soul-twisting".parse().unwrap())
            });
            if twisting && !already && ritual != "armor-of-unfeeling" {
                plan.pre_fight.push(PreCast {
                    slot: slot.name.clone(),
                    skill: SkillRef::Slug("soul-twisting".parse().unwrap()),
                    target: TargetRule::Caster,
                });
            }
            plan.pre_fight.push(PreCast {
                slot: slot.name.clone(),
                skill: SkillRef::Slug(ritual.parse().unwrap()),
                target: TargetRule::Caster,
            });
        }
    }

    if foe_roles.contains(&RoleTag::Healing) {
        plan.called_targets
            .push(TargetRule::FoeWithRole(RoleTag::Healing));
    }
    plan
}
