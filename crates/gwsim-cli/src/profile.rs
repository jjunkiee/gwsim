//! `gwsim profile` — account profiles (T5.7.4).
//!
//! Profiles live as RON in the user directory's `profiles/` folder. `path`
//! prints a profile's file for hand editing, which stands in for an
//! interactive editor on Windows.

use std::collections::BTreeSet;
use std::io::{self, Write};
use std::path::PathBuf;

use gwsim_data::core::TitleTrack;
use gwsim_data::ids::{SkillId, Slug};
use gwsim_data::profile::{AccountProfile, Unlocks};
use gwsim_data::user_dir::UserDir;

use crate::data::{FAILED, OK};
use crate::evaluate::Failure;
use crate::{ProfileArgs, ProfileCommand};

fn profiles_dir(args: &ProfileArgs) -> Result<PathBuf, String> {
    let dir = UserDir::resolve(args.user_dir.as_deref())
        .ok_or("no user directory: pass --user-dir or set GWSIM_USER_DIR")?;
    Ok(dir.subfolder("profiles"))
}

/// Runs `gwsim profile`.
pub fn run(args: &ProfileArgs, out: &mut impl Write) -> io::Result<i32> {
    match profile(args, out) {
        Ok(code) => Ok(code),
        Err(Failure::Io(error)) => Err(error),
        Err(Failure::Message(message)) => {
            writeln!(out, "{message}")?;
            Ok(FAILED)
        }
    }
}

fn check_name(name: &str) -> Result<(), String> {
    name.parse::<Slug>()
        .map(|_| ())
        .map_err(|_| format!("{name:?}: a profile name is lower-case letters, digits and hyphens"))
}

fn profile(args: &ProfileArgs, out: &mut impl Write) -> Result<i32, Failure> {
    let dir = profiles_dir(args)?;
    let path_of = |name: &str| AccountProfile::path_in(&dir, name);
    let load = |name: &str| -> Result<AccountProfile, String> {
        check_name(name)?;
        AccountProfile::read(&path_of(name))
    };
    match &args.command {
        ProfileCommand::List => {
            let mut names: Vec<String> = std::fs::read_dir(&dir)
                .map(|entries| {
                    entries
                        .filter_map(|e| e.ok())
                        .filter_map(|e| {
                            e.path()
                                .file_stem()
                                .filter(|_| e.path().extension().is_some_and(|x| x == "ron"))
                                .map(|s| s.to_string_lossy().into_owned())
                        })
                        .collect()
                })
                .unwrap_or_default();
            names.sort();
            if names.is_empty() {
                writeln!(out, "no profiles in {}", dir.display())?;
            }
            for name in names {
                writeln!(out, "{name}")?;
            }
        }
        ProfileCommand::Show { name } => {
            let profile = load(name)?;
            let text = ron::ser::to_string_pretty(&profile, ron::ser::PrettyConfig::default())
                .map_err(|e| e.to_string())?;
            writeln!(out, "{text}")?;
        }
        ProfileCommand::New { name } => {
            check_name(name)?;
            let path = path_of(name);
            if path.exists() {
                return Err(Failure::Message(format!(
                    "{} already exists",
                    path.display()
                )));
            }
            let profile = AccountProfile {
                name: name.clone(),
                ..AccountProfile::default()
            };
            profile.write(&path)?;
            writeln!(out, "created {}", path.display())?;
        }
        ProfileCommand::Path { name } => {
            check_name(name)?;
            writeln!(out, "{}", path_of(name).display())?;
        }
        ProfileCommand::Set {
            name,
            what,
            track,
            rank,
        } => {
            if what != "title" {
                return Err(Failure::Message(format!(
                    "profile set {what:?}: only `title` can be set"
                )));
            }
            let mut profile = load(name)?;
            let track = TitleTrack::ALL
                .into_iter()
                .find(|t| {
                    format!("{t:?}").eq_ignore_ascii_case(&track.replace([' ', '-', '_'], ""))
                })
                .ok_or_else(|| {
                    format!(
                        "no title track {track:?}; tracks are {}",
                        TitleTrack::ALL.map(|t| format!("{t:?}")).join(", ")
                    )
                })?;
            profile.title_ranks.insert(track, *rank);
            profile.write(&path_of(name))?;
            writeln!(out, "{name}: {track:?} rank {rank}")?;
        }
        ProfileCommand::Unlock { name, what, skills }
        | ProfileCommand::Lock { name, what, skills } => {
            if what != "skill" {
                return Err(Failure::Message(format!(
                    "{what:?}: only `skill` can be locked or unlocked"
                )));
            }
            let unlocking = matches!(args.command, ProfileCommand::Unlock { .. });
            let data = crate::loading::load(args.data_dir.as_deref())
                .map_err(|(origin, problems)| format!("could not read {origin}:\n{problems}"))?
                .data;
            let mut ids = Vec::new();
            for text in skills {
                let skill = text
                    .parse::<Slug>()
                    .ok()
                    .and_then(|slug| data.skill(&slug))
                    .or_else(|| {
                        text.parse::<u16>()
                            .ok()
                            .and_then(|n| data.skill_by_id(SkillId(n)))
                    })
                    .ok_or_else(|| format!("no skill {text:?} in the data"))?;
                ids.push(skill.id);
            }
            let mut profile = load(name)?;
            let every: BTreeSet<SkillId> = data.skills.values().map(|e| e.value.id).collect();
            let mut set = match &profile.unlocked_skills {
                Unlocks::All => every,
                Unlocks::Only(set) => set.clone(),
            };
            for id in &ids {
                if unlocking {
                    set.insert(*id);
                } else {
                    set.remove(id);
                }
            }
            profile.unlocked_skills = Unlocks::Only(set);
            profile.write(&path_of(name))?;
            writeln!(
                out,
                "{name}: {} {} skill(s)",
                if unlocking { "unlocked" } else { "locked" },
                ids.len()
            )?;
        }
        ProfileCommand::Hero { name, action, hero } => {
            let data = crate::loading::load(args.data_dir.as_deref())
                .map_err(|(origin, problems)| format!("could not read {origin}:\n{problems}"))?
                .data;
            let heroes: BTreeSet<Slug> = data
                .heroes
                .as_ref()
                .map(|h| h.value.heroes.iter().map(|x| x.slug.clone()).collect())
                .unwrap_or_default();
            let slug: Slug = hero
                .parse()
                .ok()
                .filter(|s| heroes.contains(s))
                .ok_or_else(|| format!("no hero {hero:?} in creatures/heroes.ron"))?;
            let mut profile = load(name)?;
            let mut owned = match &profile.heroes_owned {
                Unlocks::All => heroes,
                Unlocks::Only(set) => set.clone(),
            };
            match action.as_str() {
                "add" => {
                    owned.insert(slug.clone());
                }
                "remove" => {
                    owned.remove(&slug);
                }
                other => {
                    return Err(Failure::Message(format!(
                        "hero {other:?}: use add or remove"
                    )));
                }
            }
            profile.heroes_owned = Unlocks::Only(owned);
            profile.write(&path_of(name))?;
            let done = if action == "add" { "added" } else { "removed" };
            writeln!(out, "{name}: hero {slug} {done}")?;
        }
    }
    Ok(OK)
}
