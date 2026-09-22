# Attribution and licensing for gwsim data

*This file describes everything under `data/`. The code in `crates/` is licensed
separately — see the `LICENSE` file at the root of the repository.*

## What this data is

`data/` holds **game facts** and **gwsim's own encodings of them**:

- numbers: costs, activation and recharge times, durations, ranges, attribute scaling;
- names and identifiers: skills, attributes, professions, foes, areas;
- relationships: which skill belongs to which attribute, which foe appears in which area;
- gwsim's own effect encodings, written in this project's effect DSL, which describe what a
  skill *does* in terms the simulator can execute.

The effect encodings are original work. They are an interpretation of observed game
behaviour, not a transcription of anything.

## Source of the facts

The facts come from the **[Guild Wars Wiki](https://wiki.guildwars.com/)**.

Every data file carries a `provenance` block naming the wiki pages its values came from,
along with the date they were read and a review status. That block is the per-file
attribution; this file is the collective one.

**The facts are expressed originally.** The wiki's own copyright policy holds that factual
information taken from a source under an incompatible licence may be used only if it is
expressed originally, and that this does not extend to images. gwsim applies the same rule
to the wiki itself: we take the facts and express them in our own form, because the wiki's
editor text is licensed under the **GNU Free Documentation License**, which is not
compatible with the CC BY-SA 4.0 licence used here.

## What is deliberately not included

- **No in-game description text.** Skill and item descriptions shown by gwsim are
  *generated* from the effect encodings. They are not ArenaNet's wording.
- **No icons, screenshots, or other art.** None, in any form, from any source.
- **No copied wiki prose.** Not in data files, not in comments, not in provenance blocks.
- **No cached pages.** The wiki extractor's cache of fetched HTML is never committed.
- **Nothing from the wiki's `Feedback:` namespace as text**, since contributions there are
  assigned to ArenaNet rather than licensed to the public.

If you believe something here crosses one of those lines, please open an issue; it will be
treated as a bug and fixed.

## Trademarks and affiliation

Guild Wars, ArenaNet, NCSOFT and the associated names and logos are trademarks or
registered trademarks of NCSOFT Corporation. They are used here only to describe the game
this tool simulates.

**gwsim is a fan project. It is not affiliated with, endorsed by, or sponsored by ArenaNet
or NCSOFT.** No ArenaNet or NCSOFT employee has reviewed this data, and no warranty of
accuracy is offered or implied.

## Accuracy

Wiki articles are written by volunteers and are not consistently reviewed by the game's
developers. Values here are therefore treated as claims to be checked, not as ground truth.
Each file records a review status:

| Status | Meaning |
| --- | --- |
| `NumbersOnly` | Values were taken from the wiki; nobody has checked the behaviour. |
| `Draft` | The effect encoding exists but has not been reviewed. |
| `Reviewed` | A person has checked the encoding against the game's behaviour. |

## Reusing gwsim data

The contents of `data/` are licensed under the **Creative Commons
Attribution-ShareAlike 4.0 International** licence (CC BY-SA 4.0). The full legal code is
in [`LICENSE`](LICENSE) in this directory, and online at
<https://creativecommons.org/licenses/by-sa/4.0/>.

You are free to share and adapt this data, including commercially, provided you give
credit and license your adaptations under the same terms.

Suggested credit:

> gwsim game data, by the gwsim contributors, licensed under
> [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/).

Note that this licence covers **gwsim's data only**. It does not and cannot grant any
rights in ArenaNet's or NCSOFT's intellectual property.

---

*Not legal advice. If you intend to redistribute this data, satisfy yourself that you
understand the terms.*
