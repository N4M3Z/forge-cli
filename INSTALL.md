# forge-cli

> Assemble, validate, and deploy markdown content across AI coding providers.

I want you to install forge-cli for me. Execute all the steps below autonomously.

## OBJECTIVE

Build the `forge` binary from source and make it available on PATH.

## DONE WHEN

Running `forge --help` prints the CLI usage and exits successfully.

## TODO

- [ ] Ensure Rust toolchain is installed
- [ ] Clone the repository
- [ ] Build the release binary
- [ ] Symlink to PATH
- [ ] Verify the installation

## Steps

### Rust toolchain

You need the Rust toolchain. If `cargo` is not available, install it:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

### Clone

```sh
git clone https://github.com/N4M3Z/forge-cli.git
cd forge-cli
```

If already cloned, pull latest:

```sh
git pull
```

### Build

```sh
cargo build --release
```

### Symlink

You must ensure `~/.local/bin` is on your PATH.

```sh
mkdir -p ~/.local/bin
ln -sf "$(pwd)/target/release/forge" ~/.local/bin/forge
```

### Verify

You must confirm the binary is accessible:

```sh
forge --help
```

EXECUTE NOW: Complete the above TODO list to achieve: `forge --help` prints CLI usage and exits successfully.
