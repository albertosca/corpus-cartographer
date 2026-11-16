🇺🇸 [English](README.md) · 🇧🇷 [Português](README.pt.md)

[![CI](https://github.com/albertosca/corpus-cartographer/actions/workflows/ci.yml/badge.svg)](https://github.com/albertosca/corpus-cartographer/actions/workflows/ci.yml)
[![Lint](https://github.com/albertosca/corpus-cartographer/actions/workflows/lint.yml/badge.svg)](https://github.com/albertosca/corpus-cartographer/actions/workflows/lint.yml)
[![Coverage](https://img.shields.io/badge/coverage-98.78%25-brightgreen)](https://github.com/albertosca/corpus-cartographer/actions/workflows/ci.yml)

# corpus-cartographer

A command-line engine that lays out a corpus as a map. It reads an `engine-export` JSON document (themes, items, and the terms each item has) and optionally a directory of term embeddings, and writes the links between items plus two map specs that [`corpus-map`](https://github.com/albertosca/corpus-map) can draw.

## Install and run

```sh
cargo install --path .
corpus-cartographer --input examples/hiking-trails.json --out-dir /tmp/trails-map
```

Without `--embeddings-dir` each term is its own axis, so two items are alike only when they share terms. With a directory holding `vocab.json` and `embeddings.safetensors`, similar terms count as alike too ([how to build one](#embeddings)). Link tuning: `--floor` and `--max-links`.

## Embeddings

The engine reads any directory with two files. `vocab.json` maps each term to a row number. `embeddings.safetensors` holds one f32 tensor named `embeddings`, with one row per term. The rows do not need to be normalized, because the engine divides each one by its norm. Terms that are missing from the vocabulary are counted in an `unknown_terms` warning.

The quickest way to build that directory is to encode your own terms with a static [Model2Vec](https://github.com/MinishLab/model2vec) model, which needs no GPU and no PyTorch. Save this as `build_embeddings.py`:

```python
import json
import sys
from pathlib import Path

from model2vec import StaticModel
from safetensors.numpy import save_file

export_path, model_name, output_directory = sys.argv[1], sys.argv[2], Path(sys.argv[3])
export = json.loads(Path(export_path).read_text())
terms = sorted(
    {term for theme in export["themes"] for term in theme["terms"]}
    | {entry["term"] for item in export["items"] for entry in item["terms"]}
)
vectors = StaticModel.from_pretrained(model_name).encode(terms).astype("float32")
output_directory.mkdir(parents=True, exist_ok=True)
(output_directory / "vocab.json").write_text(json.dumps({term: index for index, term in enumerate(terms)}))
save_file({"embeddings": vectors}, output_directory / "embeddings.safetensors")
print(f"{len(terms)} terms, {vectors.shape[1]} dimensions")
```

```sh
pip install model2vec
python build_embeddings.py examples/hiking-trails.json minishlab/potion-base-8M /tmp/trails-embeddings
corpus-cartographer --input examples/hiking-trails.json --embeddings-dir /tmp/trails-embeddings --out-dir /tmp/trails-map
```

On the trails example the map goes from 6 links to 14. The mountain trails link to each other even where they share few terms, and so do the coast trails. Run the script again whenever the export gains terms. Any of these models works in its place:

| Model | Download | Languages | Licence |
|---|---|---|---|
| [`minishlab/potion-base-8M`](https://huggingface.co/minishlab/potion-base-8M) | 30 MB | English | MIT |
| [`minishlab/potion-base-32M`](https://huggingface.co/minishlab/potion-base-32M) | 129 MB | English | MIT |
| [`minishlab/potion-multilingual-128M`](https://huggingface.co/minishlab/potion-multilingual-128M) | 512 MB | 101, Portuguese included | MIT |

Use the multilingual model when the terms are not in English. "Praia" and "litoral" (beach and coast) score 0.14 with the English model and 0.64 with the multilingual one.

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

The output follows the input version. Export v2 (the earlier names) is still accepted in this release and is removed in 0.5.0.

## Works with

It pairs with the `corpus_atlas` Rails engine, which produces the export and imports the links, and with the `corpus-map` viewer, which draws the map. Each also works alone.

## Development

Rust 1.88 or newer. CI runs `cargo test`, `cargo fmt --check`, `cargo clippy` with the pedantic group, `cargo deny check` (advisories, licences, sources), the suite on Rust 1.88, and `cargo llvm-cov` with a floor of 98% of lines. The coverage badge shows the measured number and is updated by hand when it changes.

## Licence

MIT.
