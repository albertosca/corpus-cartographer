🇺🇸 [English](README.md) · 🇧🇷 [Português](README.pt.md)

[![CI](https://github.com/albertosca/corpus-cartographer/actions/workflows/ci.yml/badge.svg)](https://github.com/albertosca/corpus-cartographer/actions/workflows/ci.yml)
[![Lint](https://github.com/albertosca/corpus-cartographer/actions/workflows/lint.yml/badge.svg)](https://github.com/albertosca/corpus-cartographer/actions/workflows/lint.yml)
[![Coverage](https://img.shields.io/badge/coverage-98.78%25-brightgreen)](https://github.com/albertosca/corpus-cartographer/actions/workflows/ci.yml)

# corpus-cartographer

**Turn a pile of tagged items into a map: which items are related, why, and where each one sits.**

You give it items (articles, support tickets, papers, hiking trails) that each carry a few terms and sit in a tree of themes. It finds each item's closest neighbours and explains every link by the terms the two share. It also places the themes and items on a 2D map. The output is plain JSON. You can use the links on their own, or hand the map specs to the [`corpus-map`](https://github.com/albertosca/corpus-map) React library, which draws them as a zoomable landscape ([live demo](https://albertosca.github.io/corpus-map/)).

It is a single binary. The same input always gives the same output, and it reads the last run's positions, so the map keeps its shape as the corpus grows. The 15-item example runs in under 10 ms.

```
export.json ──▶ corpus-cartographer ──▶ links.json            links, positions, warnings
(+ embeddings,                       ├─▶ map-spec.v1.json      ──▶ corpus-map
   optional)                         └─▶ map-spec.nested.json  ──▶ corpus-map
```

## Quick start

Download a binary for macOS (Apple silicon or Intel) or Linux x86_64 from the [latest release](https://github.com/albertosca/corpus-cartographer/releases/latest). To build from source you need Rust 1.88 or newer ([rustup](https://rustup.rs)):

```sh
git clone https://github.com/albertosca/corpus-cartographer
cd corpus-cartographer
cargo install --path .
corpus-cartographer --input examples/hiking-trails.json --out-dir trails-map
```

It prints a one-line summary and writes three files to `trails-map/`:

```
items=15 placed=15 waiting=0 with_vector=14 links=6 warnings=0
```

Each link in `trails-map/links.json` says how alike two items are (cosine similarity, from -1 to 1) and which terms they share:

```json
{
  "source": 101,
  "target": 102,
  "similarity": 0.4894,
  "crosses_theme": false,
  "shared": [
    { "term": "ridge", "source_labels": ["ridge"], "target_labels": ["ridge"] },
    { "term": "summit", "source_labels": ["summit"], "target_labels": ["summit"] },
    { "term": "wind", "source_labels": ["wind"], "target_labels": ["wind"] }
  ]
}
```

To see the map, install `corpus-map` from npm and pass the contents of `trails-map/map-spec.nested.json` to its `<CorpusMap spec={...} />` component, as the [corpus-map README](https://github.com/albertosca/corpus-map#usage) shows.

## Your own data

The engine does not read documents or extract terms, so you supply them. The terms can be keywords, tags, categories or extracted entities, whatever your system already has. Themes form a tree of top-level themes and their subthemes, and each item sits in one subtheme. This is a complete export:

```json
{
  "schema": "corpus-cartographer/engine-export",
  "version": 3,
  "taxonomy_version": 1,
  "themes": [
    { "id": 1, "parent_id": null, "name": "Databases", "terms": [] },
    { "id": 11, "parent_id": 1, "name": "Indexing", "terms": [] },
    { "id": 2, "parent_id": null, "name": "Networking", "terms": [] },
    { "id": 21, "parent_id": 2, "name": "Protocols", "terms": [] }
  ],
  "items": [
    { "id": 1, "title": "B-trees from scratch", "subtheme_id": 11, "status": "assigned", "badges": [],
      "terms": [{ "label": "B-tree", "term": "b-tree" }, { "label": "index", "term": "index" }, { "label": "storage", "term": "storage" }] },
    { "id": 2, "title": "Why my query ignores the index", "subtheme_id": 11, "status": "assigned", "badges": [],
      "terms": [{ "label": "index", "term": "index" }, { "label": "query plan", "term": "query plan" }, { "label": "storage", "term": "storage" }] },
    { "id": 3, "title": "TCP slow start", "subtheme_id": 21, "status": "assigned", "badges": [],
      "terms": [{ "label": "TCP", "term": "tcp" }, { "label": "congestion", "term": "congestion" }, { "label": "latency", "term": "latency" }] },
    { "id": 4, "title": "QUIC in one page", "subtheme_id": 21, "status": "assigned", "badges": [],
      "terms": [{ "label": "QUIC", "term": "quic" }, { "label": "congestion", "term": "congestion" }, { "label": "latency", "term": "latency" }] }
  ],
  "previous": null,
  "rejected_pairs": []
}
```

```sh
corpus-cartographer --input articles.json --out-dir articles-map --floor 0.2
```

This links the two database articles to each other, and the two networking articles to each other. The `--floor 0.2` is there because the corpus is tiny. A term weighs more the fewer items use it, and what every item shares counts for nothing. On four items that keeps similarities low, and the default floor of 0.4 would draw no link. The 15 trails of the quick start already link with the default.

| Option | Default | What it does |
|---|---|---|
| `--input` | required | the export to read |
| `--out-dir` | required | where the three output files go, created if missing |
| `--embeddings-dir` | none | let related terms count as alike, see [below](#better-links-with-embeddings) |
| `--floor` | `0.4` | the lowest cosine similarity that still makes a link, between -1 and 1 |
| `--max-links` | `3` | the most links kept per item; links to other themes are tried first |

## Better links with embeddings

This is optional. Without embeddings, two items are alike only when they use the very same terms. With embeddings, related terms count too: "beach" and "shore" have a cosine similarity of 0.54 with `potion-base-8M`.

The engine reads a directory with two files. `vocab.json` maps each term to a row number. `embeddings.safetensors` holds one f32 tensor named `embeddings`, with one row per term. The engine normalizes the rows itself. Terms that are missing from the vocabulary are counted in an `unknown_terms` warning.

[`scripts/build_embeddings.py`](scripts/build_embeddings.py) builds that directory from your export with a static [Model2Vec](https://github.com/MinishLab/model2vec) model, which needs no GPU and no PyTorch:

```sh
pip install model2vec
python scripts/build_embeddings.py examples/hiking-trails.json minishlab/potion-base-8M trails-embeddings
corpus-cartographer --input examples/hiking-trails.json --embeddings-dir trails-embeddings --out-dir trails-map
```

On the trails example this takes the map from 6 links to 14. The mountain trails now link to each other even where they share fewer terms, and so do the coast trails. Run the script again whenever the export gains terms. Any of these models works:

| Model | Download | Languages | Licence |
|---|---|---|---|
| [`minishlab/potion-base-8M`](https://huggingface.co/minishlab/potion-base-8M) | 30 MB | English | MIT |
| [`minishlab/potion-base-32M`](https://huggingface.co/minishlab/potion-base-32M) | 129 MB | English | MIT |
| [`minishlab/potion-multilingual-128M`](https://huggingface.co/minishlab/potion-multilingual-128M) | 512 MB | 101, Portuguese included | MIT |

Use the multilingual model when the terms are not in English. "Praia" and "litoral" (beach and coast) have a cosine similarity of 0.14 with the English model and 0.64 with the multilingual one.

## Output

| File | What it is |
|---|---|
| `links.json` | `corpus-cartographer/engine-links`: links between items, positions, warnings, counts |
| `map-spec.v1.json` | territories and nodes for `corpus-map` |
| `map-spec.nested.json` | the same with nested territories, plus the links as edges |

## How it works, and its limits

- **Item vectors.** Each item's vector is the weighted mean of its terms' vectors, minus the corpus mean. A term weighs more the fewer items use it, so what every item shares counts for nothing. Links are the cosine between two item vectors.
- **Layout.** The layout runs classical multidimensional scaling on 1 − cosine at each level: top-level themes, the subthemes inside them, and the items inside each subtheme. It is closed-form, with no random seed, so the same input gives the same bytes. The trade-off is that a 2D picture distorts some distances, so the map is a view of the corpus and not a measurement.
- **Stability.** When `previous` holds the last run's positions, each level is fitted onto them by rotation, translation and, when that fits strictly better, reflection. A new item therefore does not spin or mirror the map. The tests hold this to a bound: adding one item moves the untouched territories by less than 10% of the map's extent.
- **Waiting items.** An item whose status is not `assigned` waits in a row below the map, and the summary counts it as `waiting`.
- **Size.** Without embeddings each term is an axis, and the engine refuses more than 4,000,000 cells (the term count times the larger of the term count and the count of items with terms) and asks for embeddings instead. The 5,000-item corpus below hits that limit without embeddings. With embeddings, time grows roughly with the square of the item count. This was measured on an Apple M1 running other work, with synthetic items in 10 themes of 5 subthemes, 5 terms each, and 256-dimension embeddings:

  | Items | Time | Peak memory |
  |---|---|---|
  | 1,000 | 0.3 s | 21 MB |
  | 2,000 | 1.2 s | 34 MB |
  | 5,000 | 10 s | 69 MB |

  It is built for corpora of thousands of items, the scale `corpus-map` draws, and not for millions.
- **Warnings.** Warnings are listed in `links.json` and never stop a run:
  - `unknown_terms`: some terms are missing from the vocabulary, with a count.
  - `no_flavour`: a theme has no vector, so it is placed on a ring around its parent.
  - `no_alignment`: a level could not be fitted to `previous` even though it shares territories with it.
- **Errors.** A malformed export, a missing file or a bad embeddings directory exits with status 1 and an `error:` line naming the file and the problem, and writes nothing.

## Contract v3

The input is `corpus-cartographer/engine-export`, version 3. The schema is [`contracts/engine-export.v3.schema.json`](contracts/engine-export.v3.schema.json), and the output schemas sit beside it.

| Field | Meaning |
|---|---|
| `schema`, `version` | `"corpus-cartographer/engine-export"` and `3` |
| `taxonomy_version` | the producer's own version of its theme tree, or null; the engine does not use it |
| `themes[]` | `id`, `parent_id` (null for a top-level theme), `name`, `terms` (strings that describe the theme) |
| `items[]` | `id`, `title`, `subtheme_id`, `status` (`assigned`, `needs_owner`, `unclassified`), `badges`, `terms` |
| `items[].badges` | short texts to show on the item's map node; the engine copies them and never weighs them |
| `items[].terms[]` | `label` (the producer's own name for it) and `term` (the word compared; null when there is none) |
| `previous` | the positions `links.json` gave last time, or null, so the map keeps its shape |
| `rejected_pairs` | item pairs, lower id first, that must never be linked |

The output version follows the input version. Export v2 is still read in 0.4 and is removed in 0.5.0.

## Works with

- [`corpus-map`](https://github.com/albertosca/corpus-map) draws the map specs.
- `corpus_atlas`, a Rails engine, writes the export from an app's database and reads the links back.

Any program that writes the JSON above works just as well, and each piece also works alone.

## Contributing

### Run what CI runs

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

### Where things live

A run goes through these modules in order:

| Step | Module | What it does |
|---|---|---|
| 1 | `src/export.rs` | parses and checks the export (`src/legacy_v2.rs` reads v2) |
| 2 | `src/embeddings.rs` | loads the embeddings directory, or one axis per term |
| 3 | `src/vectors.rs` | builds item vectors and term weights |
| 4 | `src/layout.rs`, `src/geometry.rs` | places themes and items (MDS, alignment to `previous`) |
| 5 | `src/links.rs` | finds and explains links (`--floor`, `--max-links`) |
| 6 | `src/output.rs`, `src/warning.rs` | writes the three documents and the warnings |

`src/main.rs` is the command line, and each file opens with a comment saying what it owns.

### Changing the contract

The schemas, the example exports and the golden `engine-links` outputs live in `contracts/`, and the golden map spec of the trails example lives in `tests/golden/`. A change to what the engine reads or writes goes like this:

1. Update the schema.
2. Regenerate the goldens with `UPDATE_GOLDEN=1 cargo test`.
3. Read the diff, and commit the schema and the goldens together.

The parser refuses any input field the schema does not name, so a new input field needs a change in `src/export.rs` too. A separate test keeps the vocabulary of the first application that used the engine out of the code, the v3 documents, the tests and both READMEs; only the v2 reader may use it.

A change that someone could see goes into `Unreleased` in the [CHANGELOG](CHANGELOG.md), and the README changes go into both languages. To release, set the version in `Cargo.toml`, move `Unreleased` under that version, and push the tag `vX.Y.Z`. `release.yml` refuses a tag that differs from the crate version, then builds the three binaries.

### What the tests guarantee

The crate has five runtime dependencies (clap, serde, serde_json, safetensors, sha2), and `unsafe` code is forbidden. The tests check four guarantees on every wire format the engine reads:

- the same input gives the same output, byte for byte;
- a territory's identity does not depend on input order;
- the stability bound above holds;
- edge-case corpora still give valid output: an empty corpus, a single item, every item waiting, and items with no terms.

They also check the outputs against the JSON schemas in `contracts/` and against the golden files, and they check that malformed embeddings files are rejected. `cargo deny` checks advisories, licences and sources.

## Status

Version 0.4.1, before 1.0. The export contract can still change between minor versions. Each change is versioned and listed in the [CHANGELOG](CHANGELOG.md), and the previous version is still read for a while.

## Licence

MIT.
