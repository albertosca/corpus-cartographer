# Changelog

## Unreleased

- The minimum Rust version is 1.88, not 1.85: `sha2` 0.11 needs 1.88, so the engine never built on the version `Cargo.toml` declared. CI now runs the suite on 1.88, so the number is measured.
- CI enforces line coverage of `src/` with `cargo llvm-cov` (floor 97%, measured 97.76%) and checks every dependency, dev ones included, with `cargo deny` (RustSec advisories, licences, sources; rules in `deny.toml`).
- A document whose `version` is missing, not a whole number or out of range is now refused naming the versions the engine reads (`version 3 or ... version 2`), instead of a serde message about a field.
- A release workflow builds `corpus-cartographer` for Linux x86_64 and macOS (Apple silicon and Intel) when a `v*` tag is pushed, and attaches a tarball and a SHA-256 file per target. The tag must equal the version in `Cargo.toml`. Running it by hand builds and packages without creating a release.
- A `gem.yml` workflow runs the `corpus_atlas` gem's own suite against the engine built from the commit and its contracts (needs the `GEM_READ_TOKEN` secret and fails without it), so a change the gem cannot read fails here.

## 0.4.0

- New repository: `corpus-cartographer`, the generic map engine, started from the `recipes-engine` 0.3.1 code with a clean history.
- Contract v3 (`corpus-cartographer/engine-export` and `engine-links`, version 3): `items` instead of `recipes`, `terms` with a `label` and a `term` instead of `ingredients`, theme `terms` instead of `epicure_terms`, and a `badges` list per item instead of the quantity seal.
- Export v2 is still accepted and is removed in 0.5.0. The output follows the input version: a v2 export gets links v2 and `recipe:N` ids, byte-identical to 0.3.1 except the `engine_version` line; a v3 export gets links v3 (`counts.items`, `source_labels`, `target_labels`, `source_label`, `target_label`) and `item:N` ids.
- `--embeddings-dir` replaces `--epicure-dir`; the old name stays as an alias.
- An item with an empty badge is refused.
