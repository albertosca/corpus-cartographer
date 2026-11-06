🇺🇸 [English](README.md) · 🇧🇷 [Português](README.pt.md)

[![CI](https://github.com/albertosca/corpus-cartographer/actions/workflows/ci.yml/badge.svg)](https://github.com/albertosca/corpus-cartographer/actions/workflows/ci.yml)
[![Lint](https://github.com/albertosca/corpus-cartographer/actions/workflows/lint.yml/badge.svg)](https://github.com/albertosca/corpus-cartographer/actions/workflows/lint.yml)

# corpus-cartographer

A command-line engine that lays out a corpus as a map. It reads an `engine-export` JSON document (themes, items, and the terms each item has) and optionally a directory of term embeddings, and writes the links between items plus two map specs that [`corpus-map`](https://github.com/albertosca/corpus-map) can draw.

## Install and run

```sh
cargo install --path .
corpus-cartographer --input examples/hiking-trails.json --out-dir /tmp/trails-map
```

Without `--embeddings-dir` each term is its own axis, so two items are alike only when they share terms. With a directory holding `vocab.json` and `embeddings.safetensors`, similar terms count as alike too. Link tuning: `--floor` and `--max-links`.

## Output

| File | What it is |
|---|---|
| `links.json` | `corpus-cartographer/engine-links`: links between items, positions, warnings, counts |
| `map-spec.v1.json` | territories and nodes for `corpus-map` |
| `map-spec.nested.json` | the same with nested territories and edges |

## Contract v3

The input is `corpus-cartographer/engine-export`, version 3. The schema is `contracts/engine-export.v3.schema.json` and a complete example is `contracts/engine-export.v3.json`.

| Field | Meaning |
|---|---|
| `themes[]` | `id`, `parent_id` (null for a top-level theme), `name`, `terms` |
| `items[]` | `id`, `title`, `subtheme_id`, `status` (`assigned`, `needs_owner`, `unclassified`), `badges`, `terms` |
| `items[].badges` | strings the producer wants shown on the item's map node; the engine copies them and never weighs them |
| `items[].terms[]` | `label` (what the producer calls it) and `term` (the word compared; null when unknown) |
| `previous` | positions from the last run, so the map keeps its shape |
| `rejected_pairs` | item pairs that must never be linked |

The output follows the input version. Export v2 (the recipe-era names) is still accepted in this release and is removed in 0.5.0.

## Works with

It pairs with the `corpus_atlas` Rails engine, which produces the export and imports the links, and with the `corpus-map` viewer, which draws the map. Each also works alone.

## Licence

MIT.
