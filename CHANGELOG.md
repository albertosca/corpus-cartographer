# Changelog

## Unreleased

- The layout gives each sub-theme a disk whose area grows with its count of items that have a vector, and pushes sibling disks apart until none overlap. Before, every level got 35% of the smallest gap between two siblings, so one close pair squeezed the whole map: on the arXiv run, 600 papers sat within a radius of 1.2 on a map 100 wide. Links are unchanged; positions change, and so do the goldens.
- The `gem.yml` workflow is removed: the engine is tested on its own, and the `corpus_atlas` gem checks the engine it pins from its own repository. The `GEM_READ_TOKEN` secret is no longer needed.
- `scripts/build_embeddings.py` builds an embeddings directory from an export with a Model2Vec model (a null term is skipped, and an export with no terms is refused). The module comment in `src/embeddings.rs` describes the format instead of naming a model that does not exist.
- Both READMEs are rewritten as a short front door: install, a GIF of the trails example drawn by `corpus-map`, a quick start with the real output, and links to the details. The details move to `docs/usage.md` (your own data, options, embeddings, output, warnings and errors, contract) and `docs/how-it-works.md` (why the engine exists, the design and its trade-offs, measured sizes, what the tests guarantee).
- `CONTRIBUTING.md` holds the CI commands, a module map, the contract-change steps and the release steps, and says contributions are accepted under the MIT licence. A pull request template and a bug report template are added.
- The test that keeps the first application's vocabulary out of the engine now also reads `docs/`, both READMEs and `CONTRIBUTING.md`.
- A real-corpus example: `scripts/build_arxiv_export.py` builds an export from 8,000 arXiv papers (terms from the titles), and `docs/usage.md` shows the run (9,881 links, 120 s, 171 MB) and its limits.
- `rust-toolchain.toml` no longer says Dependabot bumps it, since no Dependabot config exists; module comments in `src/output.rs` and `src/warning.rs` no longer name the host application.

## 0.4.1

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
