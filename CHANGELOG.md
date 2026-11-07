# Changelog

## Unreleased

- A release workflow builds `corpus-cartographer` for Linux x86_64 and macOS (Apple silicon and Intel) when a `v*` tag is pushed, and attaches a tarball and a SHA-256 file per target. The tag must equal the version in `Cargo.toml`. Running it by hand builds and packages without creating a release.

## 0.4.0

- New repository: `corpus-cartographer`, the generic map engine, started from the `recipes-engine` 0.3.1 code with a clean history.
- Contract v3 (`corpus-cartographer/engine-export` and `engine-links`, version 3): `items` instead of `recipes`, `terms` with a `label` and a `term` instead of `ingredients`, theme `terms` instead of `epicure_terms`, and a `badges` list per item instead of the quantity seal.
- Export v2 is still accepted and is removed in 0.5.0. The output follows the input version: a v2 export gets links v2 and `recipe:N` ids, byte-identical to 0.3.1 except the `engine_version` line; a v3 export gets links v3 (`counts.items`, `source_labels`, `target_labels`, `source_label`, `target_label`) and `item:N` ids.
- `--embeddings-dir` replaces `--epicure-dir`; the old name stays as an alias.
- An item with an empty badge is refused.
