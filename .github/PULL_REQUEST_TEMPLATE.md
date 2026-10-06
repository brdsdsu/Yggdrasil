## Summary
What does this PR change, and why?

## Type
- [ ] Bug fix
- [ ] Feature
- [ ] Performance
- [ ] Refactor
- [ ] Docs
- [ ] Test
- [ ] CI / tooling

## Changes
-

## Validation
How did you test this? For performance changes, paste before/after numbers from
the same machine and the same session (see `bench/BASELINE.md` for the noise floor).

## Checklist
- [ ] `cargo test --workspace` passes
- [ ] No new `cargo clippy` warnings in the crates touched
- [ ] Changed code is `rustfmt`-formatted (`spatial-index` excepted, see CONTRIBUTING.md)
- [ ] New public items have `///` doc comments
- [ ] Public `scene-graph` API changes are mirrored in `python-binding` and `yggdrasil.pyi`, or the PR says why not
- [ ] The book (`docs/`) is updated if behavior it describes changed
- [ ] Any `spatial-index` change from upstream carries a `FORK DEVIATION` comment and is listed in `spatial-index/README.md`
