# Contributing

[README](README.md) · [Usage](docs/usage.md) · [How it works](docs/how-it-works.md) · **Contributing**

Issues and pull requests are welcome. Contributions are accepted under the MIT licence in [LICENSE](LICENSE).

## Run what CI runs

The crate supports Rust 1.88 or newer, and `rust-toolchain.toml` pins the toolchain used to develop it (rustup installs it on first use). These are the commands from `.github/workflows/`:

```sh
cargo test --locked
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings      # pedantic lints, set in Cargo.toml
cargo install --locked cargo-deny@0.20.2 && cargo deny --locked check
rustup toolchain install 1.88 --profile minimal && cargo +1.88 test --locked
rustup component add llvm-tools-preview && cargo install --locked cargo-llvm-cov@0.9.1
cargo llvm-cov --locked --summary-only --fail-under-lines 98
```

The coverage badge is a fixed image, so update its number in both READMEs when `cargo llvm-cov` reports a new one.

## Where things live

A run goes through these modules in order:

| Step | Module | What it does |
|---|---|---|
| 1 | `src/export.rs` | parses and checks the export (`src/legacy_v2.rs` reads v2) |
| 2 | `src/embeddings.rs` | loads the embeddings directory, or one axis per term |
| 3 | `src/vectors.rs` | builds item vectors and term weights |
| 4 | `src/layout.rs`, `src/geometry.rs` | places themes and items (MDS, alignment to `previous`) |
| 5 | `src/links.rs` | finds and explains links (`--floor`, `--max-links`) |
| 6 | `src/output.rs`, `src/warning.rs` | writes the three documents and the warnings |

`src/main.rs` is the command line, and each file opens with a comment saying what it owns. [How it works](docs/how-it-works.md) explains the design behind these steps.

## Changing the contract

The schemas, the example exports and the golden `engine-links` outputs live in `contracts/`, and the golden map spec of the trails example lives in `tests/golden/`. A change to what the engine reads or writes goes like this:

1. Update the schema.
2. Regenerate the goldens with `UPDATE_GOLDEN=1 cargo test`.
3. Read the diff, and commit the schema and the goldens together.

The parser refuses any input field the schema does not name, so a new input field needs a change in `src/export.rs` too. A separate test keeps the vocabulary of the first application that used the engine out of the code, the v3 documents, the tests, the docs and both READMEs; only the v2 reader may use it.

## Changelog and releases

A change that someone could see goes into `Unreleased` in the [CHANGELOG](CHANGELOG.md), and README changes go into both languages.

To release:

1. Set the version in `Cargo.toml`.
2. Move `Unreleased` under that version.
3. Push the tag `vX.Y.Z`.

`release.yml` refuses a tag that differs from the crate version, then builds the three binaries and attaches them to the release.
