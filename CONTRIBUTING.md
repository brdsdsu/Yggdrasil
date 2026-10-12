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
cargo run -p scene-graph --example demo       # the book's walkthrough, as one program
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
| `scene-graph` | `cargo test -p scene-graph` |
| `spatial-index` | `cargo test -p spatial-index` (upstream kiddo's suite is the regression net for the fork) |
| `python-binding` | `cargo build -p python-binding`, then `maturin develop` and exercise the change from Python |
| `viz` | `cargo run -p viz`, and check the invariants in [`viz/CLAUDE.md`](viz/CLAUDE.md) still hold |
| `docs/` | `mdbook build docs` |
| Performance-sensitive code | `cargo run --release -p bench` before and after (see [Performance Work](#performance-work)) |
| Any change | `cargo test --workspace` before opening a PR |

## Code Standards

### Documentation
- Every public struct, field, enum, variant, trait and function gets a `///` doc comment.
- Doc examples refer to a crate by its **lib** name, not its package name: `scene_graph`,
  and `kiddo` inside `spatial-index`.
- If a change alters behavior the book describes, update the chapter in `docs/src/` in the
  same PR.

### Errors
- Library code returns `scene_graph::Result` and adds a `SceneGraphError` variant rather
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
