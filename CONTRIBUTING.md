# Contributing to Yggdrasil

## Getting Started

### Prerequisites

- Rust stable (the first-party crates use edition 2024; `spatial-index` needs 1.89+)
- For `viz` on Linux, Bevy's system libraries:

  ```bash
  sudo apt-get install pkg-config libx11-dev libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev
  ```

- For `python-binding`: Python 3.8+ and [uv](https://docs.astral.sh/uv/) (or pip and maturin)
- For the book: [mdBook](https://rust-lang.github.io/mdBook/) 0.5

### Local Setup

```bash
git clone https://github.com/brdsdsu/Yggdrasil.git
cd Yggdrasil
cargo build --workspace
```

### Verify Your Setup

```bash
cargo test --workspace                        # every crate's unit, integration and doc tests
cargo run -p yggdrasil-3d --example demo      # the book's walkthrough, as one program
cargo run --release -p viz                    # the visualizer on its built-in demo scene
```

### Python Bindings

```bash
cd python-binding
uv sync                                       # creates .venv with maturin
uv run maturin develop --release              # builds the extension into .venv
uv run python -c "import yggdrasil"
```

### The Book

```bash
mdbook serve docs --open                      # live-reloading preview
```

## Development Workflow

1. Create a branch from `master` (fork first if you are not a collaborator)
2. Make your changes
3. Run the checks for what you touched (see [Testing](#testing))
4. Open a PR using the pull request template
5. Make sure CI (`cargo test --workspace`) passes before merge

## Branch Naming

```
feat/<name>
fix/<name>
perf/<name>
docs/<name>
refactor/<name>
test/<name>
ci/<name>
chore/<name>
```

## Commit Messages

Write a short, lowercase summary of what the commit does. When a commit is scoped to
one crate or area, lead with it:

```
spatial-index: adaptive bulk construction
docs: bring spatial-index in line with the code
split a full leaf on its widest axis instead of on tree depth
```

Use the body to explain *why*, and say so explicitly when a commit changes public API.

## Testing

| What changed | Run |
|---|---|
| `scene-graph` | `cargo test -p yggdrasil-3d` |
| `spatial-index` | `cargo test -p yggdrasil-3d-spatial-index` (upstream kiddo's suite is the regression net for the fork) |
| `python-binding` | `cargo build -p python-binding`, then `maturin develop` and exercise the change from Python |
| `viz` | `cargo run -p viz`, and check the invariants in [`viz/CLAUDE.md`](viz/CLAUDE.md) still hold |
| `docs/` | `mdbook build docs` |
| Performance-sensitive code | `cargo run --release -p bench` before and after (see [Performance Work](#performance-work)) |
| Any change | `cargo test --workspace` before opening a PR |

## Code Standards

### Documentation
- Every public struct, field, enum, variant, trait and function gets a `///` doc comment.
- Doc examples refer to a crate by its **lib** name, not its package name: `yggdrasil`
  inside `scene-graph/` (package `yggdrasil-3d`), and `kiddo` inside `spatial-index/`
  (package `yggdrasil-3d-spatial-index`). The other workspace crates depend on
  `yggdrasil-3d` under the key `scene-graph`, so their code says `scene_graph::`.
- If a change alters behavior the book describes, update the chapter in `docs/src/` in the
  same PR.

### Errors
- Library code returns `yggdrasil::Result` and adds a `SceneGraphError` variant rather
  than panicking on bad input.

### Formatting and lints
- Run `cargo fmt -p <crate>` on the crates you touch. Do **not** run `cargo fmt --all`:
  it rewrites the vendored `spatial-index` (see below).
- Don't introduce new `cargo clippy` warnings in the crates you touch.

### Python parity
- Public `scene-graph` API should be reachable from Python. When you add or change it,
  update `python-binding/src/` and the type stub `python-binding/yggdrasil.pyi`, or say in
  the PR why it stays Rust-only.

## The Vendored `spatial-index`

`spatial-index/` is a fork of [kiddo](https://github.com/sdd/kiddo) 6.0.0-alpha.3 with an
adaptive split-dimension policy. Its [README](spatial-index/README.md) records the
provenance and every deviation from upstream. To keep future rebases tractable:

- Everything not listed as a deviation stays byte-for-byte upstream. Don't reformat it,
  fix unrelated lints in it, or modernize it.
- Every source deviation carries a `FORK DEVIATION` comment at the site
  (`git grep 'FORK DEVIATION'` lists them all) and an entry in its README.
- The fork keeps kiddo's license (MIT OR Apache-2.0) and its `LICENSE-MIT` and
  `LICENSE-APACHE` files.

## Performance Work

`bench/` links `spatial-index` and upstream kiddo into one binary and measures both on the
same inputs, in the same process. [`bench/BASELINE.md`](bench/BASELINE.md) documents the
methodology and the noise floor: deltas inside it are noise. When a PR claims a speedup,
paste before-and-after numbers from the same machine and the same session, and say what
hardware they came from.

## Releasing

Three things ship, always at one shared version, `[workspace.package] version` in the root
`Cargo.toml`:

| Registry | Name | Built from |
|---|---|---|
| crates.io | `yggdrasil-3d` | `scene-graph/` |
| crates.io | `yggdrasil-3d-spatial-index` | `spatial-index/` |
| PyPI | `yggdrasil-3d` | `python-binding/` (abi3 wheels and an sdist) |

To release:

1. Set `[workspace.package] version` in the root `Cargo.toml`. When the leading non-zero
   component changes (0.1 → 0.2, or 1.x → 2.0), also update the `version` on both
   `spatial_index` entries in `scene-graph/Cargo.toml`; cargo refuses to resolve the
   workspace until you do.
2. Merge that to `master`.
3. Tag the merge commit `v<version>` and push the tag:
   `git tag v0.2.0 && git push origin v0.2.0`.

[`release.yml`](.github/workflows/release.yml) then checks that the tag matches the
version and that the version is new on both registries, runs the CI suite on the tagged
commit, and builds and verifies both crates, every wheel and the sdist. It uploads only once
all of that passes. If an upload fails partway, use **Re-run failed jobs**, not
**Re-run all jobs**: a full re-run fails its version check on the half that already went out.

To check that every wheel still builds without releasing, run the workflow by hand from the
Actions tab: it does everything except upload.

### One-time setup

Neither registry needs a token in this repository: both trust the workflow through OIDC.

- **crates.io**: trusted publishing cannot create a crate. Publish
  `yggdrasil-3d-spatial-index` once by hand from a clean checkout of `master`
  (`cargo login`, then `cargo publish -p yggdrasil-3d-spatial-index`). Then, in each
  crate's **Settings → Trusted Publishing** (`yggdrasil-3d` and
  `yggdrasil-3d-spatial-index`), add a GitHub publisher: owner `brdsdsu`, repository
  `Yggdrasil`, workflow `release.yml`, environment `release`.
- **PyPI**: in the `yggdrasil-3d` project's **Publishing** settings, add a GitHub
  publisher with the same four values.
- **GitHub**: the `release` environment is created on the first run. Adding required
  reviewers to it in **Settings → Environments** makes every upload wait for a manual
  approval.

## Reporting Vulnerabilities

Report vulnerabilities privately as described in [SECURITY.md](SECURITY.md), never in a
public issue.

## Code of Conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md). By participating,
you agree to uphold it.

## License

Yggdrasil is licensed under the [Apache License 2.0](LICENSE). Unless you state otherwise,
any contribution you intentionally submit for inclusion is licensed under the same terms,
as described in section 5 of the license, without any additional terms or conditions.
