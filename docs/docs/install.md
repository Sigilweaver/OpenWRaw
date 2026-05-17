---
sidebar_position: 2
---

# Install

OpenWRaw ships as a Rust crate, a CLI, and a Python wheel.

## Rust

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
openwraw = "0.1"
```

Or from the command line:

```sh
cargo add openwraw
```

Build the CLI and all crates from a checkout:

```sh
git clone https://github.com/Sigilweaver/OpenWRaw
cd OpenWRaw
cargo build --workspace --release
cargo test --workspace
```

Requires Rust 1.75 or newer.

## Python

Install the wheel from PyPI:

```sh
pip install openwraw
```

From source (requires a Rust toolchain and `maturin`):

```sh
git clone https://github.com/Sigilweaver/OpenWRaw
cd OpenWRaw/crates/openwraw-py
maturin develop --release
```

## Verifying the install

Rust:

```sh
cargo test --workspace
```

Python:

```python
import openwraw
print(openwraw.__version__)
```

CLI:

```sh
openwraw inspect path/to/sample.raw
```
