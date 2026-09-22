## Summary

<!-- What does this change, and why? One paragraph is usually enough. -->

## Plan tasks

<!-- The task IDs this completes or advances, e.g. T1.2.3, T1.2.4.
     No matching task? Say so here and explain — the plan can be extended. -->

- T

## Checklist

Local checks:

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `gwsim data validate` (once it exists — T1.2.12)

Licensing — please confirm each one:

- [ ] No in-game description text or other ArenaNet wording
- [ ] No icons, screenshots or other art
- [ ] No copied or lightly reworded wiki prose
- [ ] No files from `.cache/` or `research/`

Data changes only:

- [ ] `provenance` block names the source pages and the date read
- [ ] Review status is set honestly (`NumbersOnly` / `Draft` / `Reviewed`)
- [ ] Anything unconfirmed is registered as an assumption with an ID, not guessed inline

## Notes for the reviewer

<!-- Anything you are unsure about, or deliberately left for later. -->
