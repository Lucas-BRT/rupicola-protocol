# COSMIC protocol example

Standalone protocol simulation for COSMIC. It creates a synthetic UI snapshot; it does not launch COSMIC or integrate with its toolkit.

## Dependencies

- Rust 1.99.1 and Cargo (selected by the repository's `rust-toolchain.toml`).
- `rupicola-protocol` from this repository and `serde_json` (resolved by Cargo).
- Unix domain sockets require a Unix system. No COSMIC SDK or runtime is required.

## Run

From this directory:

```sh
cargo run
cargo run -- --stdio
cargo run -- --socket /tmp/rupicola-linux_libcosmic.sock  # Unix only
```

Without arguments, prints a snapshot as JSON. `--stdio` and `--socket` serve NDJSON protocol requests.
