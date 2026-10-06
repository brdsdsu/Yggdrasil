# For Rust Ecosystem

## As a Git Dependency

Yggdrasil is not published on crates.io yet (the `yggdrasil` crate there is an
unrelated project). Depend on the `scene-graph` crate straight from the repository:

```bash
cargo add scene-graph --git https://github.com/brdsdsu/Yggdrasil
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



