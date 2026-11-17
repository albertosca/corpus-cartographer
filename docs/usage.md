# Using corpus-cartographer

[README](../README.md) · **Usage** · [How it works](how-it-works.md) · [Contributing](../CONTRIBUTING.md)

- [Your own data](#your-own-data)
- [Options](#options)
- [Better links with embeddings](#better-links-with-embeddings)
- [Output](#output)
- [Contract v3](#contract-v3)

## Your own data

The engine does not read documents or extract terms, so you supply them: keywords, tags, categories or extracted entities, whatever your system already has.

Themes form a tree of top-level themes and their subthemes, and each item sits in one subtheme. This is a complete export:

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

This links the two database articles to each other, and the two networking articles to each other.

The `--floor 0.2` is there because the corpus is tiny. A term weighs more the fewer items use it, and a term every item shares counts for nothing. On four items similarities stay low, so the default floor of 0.4 would draw no link; the 15 trails of the quick start already link with it.

To keep the map's shape on the next run, copy the `positions` object of the last `links.json` into `previous`.

## Options

| Option | Default | What it does |
|---|---|---|
| `--input` | required | the export to read |
| `--out-dir` | required | where the three output files go, created if missing |
| `--embeddings-dir` | none | let related terms count as alike, see [below](#better-links-with-embeddings) |
| `--floor` | `0.4` | the lowest cosine similarity that still makes a link, between -1 and 1 |
| `--max-links` | `3` | the most links kept per item; links to other themes are tried first |

## Better links with embeddings

This is optional. Without embeddings, two items are alike only when they use the very same terms. With embeddings, related terms count too: "beach" and "shore" have a cosine similarity of 0.54 with `potion-base-8M`.

The engine reads a directory with two files. `vocab.json` maps each term to a row number, and `embeddings.safetensors` holds one f32 tensor named `embeddings`, with one row per term. The engine normalizes the rows itself, and terms missing from the vocabulary are counted in an `unknown_terms` warning.

[`scripts/build_embeddings.py`](../scripts/build_embeddings.py) builds that directory from your export with a static [Model2Vec](https://github.com/MinishLab/model2vec) model, which needs no GPU and no PyTorch:

```sh
pip install model2vec
python scripts/build_embeddings.py examples/hiking-trails.json minishlab/potion-base-8M trails-embeddings
corpus-cartographer --input examples/hiking-trails.json --embeddings-dir trails-embeddings --out-dir trails-map
```

On the trails example this takes the map from 6 links to 14: the mountain trails now link to each other even where they share fewer terms, and so do the coast trails. Run the script again whenever the export gains terms. Any of these models works:

| Model | Download | Languages | Licence |
|---|---|---|---|
| [`minishlab/potion-base-8M`](https://huggingface.co/minishlab/potion-base-8M) | 30 MB | English | MIT |
| [`minishlab/potion-base-32M`](https://huggingface.co/minishlab/potion-base-32M) | 129 MB | English | MIT |
| [`minishlab/potion-multilingual-128M`](https://huggingface.co/minishlab/potion-multilingual-128M) | 512 MB | 101, Portuguese included | MIT |

Use the multilingual model when the terms are not in English. "Praia" and "litoral" (beach and coast) have a cosine similarity of 0.14 with the English model and 0.64 with the multilingual one.

## A real corpus: 8,000 arXiv papers

The 15 trails show the format; this shows the engine on real data. The corpus is 8,000 computer-science papers fetched from arXiv in September 2026, almost all of them from 2026. Each paper's terms are the words of its title.

The themes are 5 families (LLM agents & society, Training & inference, Vision & robotics, Theory & networks, Medical & speech AI) split into 18 subthemes. They were made before this run, by clustering the papers on a 2D projection of their abstracts' embeddings and naming each cluster.

[`scripts/build_arxiv_export.py`](../scripts/build_arxiv_export.py) turns the published sample into an export:

```sh
curl -fsSL -o arxiv-map-spec.json https://raw.githubusercontent.com/albertosca/corpus-map/main/demo/corpora/arxiv.json
python scripts/build_arxiv_export.py arxiv-map-spec.json arxiv-export.json
python scripts/build_embeddings.py arxiv-export.json minishlab/potion-base-8M arxiv-embeddings
corpus-cartographer --input arxiv-export.json --embeddings-dir arxiv-embeddings --out-dir arxiv-map
```

```
items=8000 placed=8000 waiting=0 with_vector=7999 links=9881 warnings=0
```

The run took 120 s and 171 MB on an Apple M1, and found 9,881 links, 4,284 of them between different themes. "Diffusion Models and Concept Formation" links to "Concept-Level Risk and Calibration for Governance in Diffusion Foundation Models" through `concept`, `diffusion` and `models`.

Titles are short, so the result has limits. 3,071 papers have no link at the default floor, some links rest on one rare shared word, and 1,530 links rest on the embeddings alone, with no shared term. The published sample carries titles only, so terms from the abstracts were not tried.

arXiv's metadata is available under CC0 1.0. Thank you to arXiv for use of its open access interoperability.

## Output

| File | What it is |
|---|---|
| `links.json` | `corpus-cartographer/engine-links`: links between items, positions, warnings, counts |
| `map-spec.v1.json` | territories and nodes for `corpus-map` |
| `map-spec.nested.json` | the same with nested territories, plus the links as edges |

Warnings are listed in `links.json` and never stop a run:

- `unknown_terms`: some terms are missing from the vocabulary, with a count.
- `no_flavour`: a theme has no vector, so it is placed on a ring around its parent.
- `no_alignment`: a level could not be fitted to `previous` even though it shares territories with it.

A malformed export, a missing file or a bad embeddings directory exits with status 1 and an `error:` line naming the file and the problem, and writes nothing.

## Contract v3

The input is `corpus-cartographer/engine-export`, version 3. The schema is [`contracts/engine-export.v3.schema.json`](../contracts/engine-export.v3.schema.json), and the output schemas sit beside it.

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
