# Installation

Yggdrasil is published as `yggdrasil-3d` on both [crates.io](https://crates.io/crates/yggdrasil-3d)
and [PyPI](https://pypi.org/project/yggdrasil-3d/). In both languages it imports as
`yggdrasil`, which is how every example in this book refers to it. (The `yggdrasil` names on
both registries belong to unrelated projects.)

## Rust

### From crates.io

```bash
cargo add yggdrasil-3d
```

```rust
use yggdrasil::SceneGraph;
```

It needs Rust 1.89 or newer. The API reference is on
[docs.rs](https://docs.rs/yggdrasil-3d).

### From Source

To follow unreleased changes on `master`, depend on the repository instead:

```bash
cargo add yggdrasil-3d --git https://github.com/brdsdsu/Yggdrasil
```

To build the whole workspace, including the visualizer and the book's demo:

```bash
git clone https://github.com/brdsdsu/Yggdrasil.git && cd Yggdrasil
cargo build --release --workspace
cargo run -p yggdrasil-3d --example demo    # the walkthrough in the next chapter
```

On Linux the visualizer needs Bevy's system libraries. On Debian or Ubuntu:

```bash
sudo apt-get install pkg-config libx11-dev libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev
```

## Python

### From PyPI

```bash
pip install yggdrasil-3d
```

```python
import yggdrasil as ygg
```

Prebuilt wheels cover CPython 3.8 and newer on Linux (glibc and musl) and macOS, each on
x86_64 and arm64, and on Windows x86_64. Anywhere else, pip compiles the source
distribution, which needs a Rust toolchain.

### From Source

Build and install the bindings from a checkout. pip fetches maturin, the build backend, on
its own:

```bash
git clone https://github.com/brdsdsu/Yggdrasil.git && cd Yggdrasil
pip install ./python-binding
```

When working on the bindings themselves, rebuild them in place with
[uv](https://docs.astral.sh/uv/):

```bash
cd python-binding
uv sync                              # creates .venv with maturin
uv run maturin develop --release     # rebuilds the extension into .venv
```
