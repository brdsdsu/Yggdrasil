# For Rust Ecosystem

## From crates.io

Yggdrasil is published as `yggdrasil-3d` (the `yggdrasil` crate on crates.io is an
unrelated project). Its library is named `yggdrasil`, which is how the examples in this
book import it:

```bash
cargo add yggdrasil-3d
```

To track unreleased changes, depend on the repository instead:

```bash
cargo add yggdrasil-3d --git https://github.com/brdsdsu/Yggdrasil
```

## Build From Source

```bash
cargo build --release
```


# For Python Ecosystem

## From PyPi
you can use the official ![PyPi](https://pypi.org/project/yggdrasil-3d/) package and install it directly:
```bash
pip install yggdrasil-3d
```

## Build From Source

First build the rust binaries.
```bash 
cargo build --release
```

now build the python bindings.
```bash 
# go to the binding's directory
cd python-binding

# install the python dependancies
## using uv (recommended)
uv sync
## using pip
pip install -e .

# build the bindings
## using uv
uv run maturin build --release 

## without uv
source <PATH_TO_VENV>/bin/activate
maturin build --release

# install the bindings
pip install target/wheels/*.whl
```



