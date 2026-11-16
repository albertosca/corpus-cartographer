🇺🇸 [English](README.md) · 🇧🇷 [Português](README.pt.md)

[![CI](https://github.com/albertosca/corpus-cartographer/actions/workflows/ci.yml/badge.svg)](https://github.com/albertosca/corpus-cartographer/actions/workflows/ci.yml)
[![Lint](https://github.com/albertosca/corpus-cartographer/actions/workflows/lint.yml/badge.svg)](https://github.com/albertosca/corpus-cartographer/actions/workflows/lint.yml)
[![Coverage](https://img.shields.io/badge/coverage-98.78%25-brightgreen)](https://github.com/albertosca/corpus-cartographer/actions/workflows/ci.yml)

# corpus-cartographer

**Turn a pile of tagged items into a map: which items are related, why, and where each one sits.**

## Install

Download a binary for macOS (Apple silicon or Intel) or Linux x86_64 from the [latest release](https://github.com/albertosca/corpus-cartographer/releases/latest). To build from source you need Rust 1.88 or newer ([rustup](https://rustup.rs)):

```sh
git clone https://github.com/albertosca/corpus-cartographer
cd corpus-cartographer
cargo install --path .
```

![The hiking-trails example drawn by corpus-map: three territories, then a zoom into Mountains down to single trails](docs/trails-map.gif)

The [`corpus-map` live demo](https://albertosca.github.io/corpus-map/) shows the kind of map the output draws.

## Quick start

```sh
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

```
export.json ──▶ corpus-cartographer ──▶ links.json            links, positions, warnings
(+ embeddings,                       ├─▶ map-spec.v1.json      ──▶ corpus-map
   optional)                         └─▶ map-spec.nested.json  ──▶ corpus-map
```

## What you get

- **Explained links.** Each item's closest neighbours, each link with the terms the two share.
- **A map.** Themes, subthemes and items placed in 2D, with items that are still waiting for a theme in a row below.
- **A map that keeps its shape.** Pass the last run's positions back in, and a new item does not spin or mirror the map.
- **Same input, same bytes.** A single binary with no random seed and five runtime dependencies; the 15-item example runs in under 10 ms and 5,000 items with embeddings take about 10 s.
- **Your own terms.** Keywords, tags or entities you already have, optionally with embeddings so related terms count as alike.
- **Plain JSON.** A versioned contract with JSON schemas, so any program can write the input and read the output.

## Documentation

- [Usage](docs/usage.md): a complete minimal export, the options, embeddings, the output and the contract.
- [How it works](docs/how-it-works.md): why it exists, the design and its trade-offs, measured sizes, what the tests guarantee.
- [Contributing](CONTRIBUTING.md): the CI commands, where things live, changing the contract, releases.

## Works with

- [`corpus-map`](https://github.com/albertosca/corpus-map) draws the map specs.
- `corpus_atlas`, a Rails engine, writes the export from an app's database and reads the links back.

Any program that writes the JSON works just as well, and each piece also works alone.

## Status

Version 0.4.1, before 1.0. The export contract can still change between minor versions, each change is listed in the [CHANGELOG](CHANGELOG.md), and the previous version is still read for a while.

## Licence

MIT.
