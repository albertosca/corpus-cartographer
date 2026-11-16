# How corpus-cartographer works

[README](../README.md) · [Usage](usage.md) · **How it works** · [Contributing](../CONTRIBUTING.md)

## Why it exists

The engine started inside a personal app that maps a collection of about 200 dishes saved from videos, to see at a glance which dishes are related and why. It had to be very fast and ship as one binary with no Python, so it is written in Rust. A prototype first proved that Rust could read the embedding weights directly and give exactly the same nearest neighbours as the Python loader.

Fully automatic maps were tried first, with PCA, UMAP, k-means and HDBSCAN on 88 items. None of them gave territories a person could name without effort, and HDBSCAN found only noise in the embedding space. So the themes are an input, written by a person or a classifier, and the engine does what the experiment showed works:

- links between neighbours, explained by the terms they share;
- a stable layout inside the themes it is given.

The default floor of 0.4 is the one chosen on that real corpus of 212 items, where it gave 534 links and left 18 items unlinked. Once it worked, the engine was made generic and moved to its own repository.

## The design, and its trade-offs

- **Item vectors.** Each item's vector is the weighted mean of its terms' vectors, minus the corpus mean. A term weighs more the fewer items use it, so what every item shares counts for nothing. Links are the cosine between two item vectors.
- **Layout.** Classical multidimensional scaling on 1 − cosine at each level: top-level themes, the subthemes inside them, and the items inside each subtheme. It is closed-form, with no random seed, so the same input gives the same bytes. The trade-off is that a 2D picture distorts some distances, so the map is a view of the corpus and not a measurement.
- **Stability.** When `previous` holds the last run's positions, each level is fitted onto them by rotation, translation and, when that fits strictly better, reflection. A new item therefore does not spin or mirror the map. The tests hold this to a bound: adding one item moves the untouched territories by less than 10% of the map's extent.
- **Waiting items.** An item whose status is not `assigned` waits in a row below the map, and the summary counts it as `waiting`.

## Size

Without embeddings each term is an axis, and the engine refuses more than 4,000,000 cells (the term count times the larger of the term count and the count of items with terms) and asks for embeddings instead. The 5,000-item corpus below hits that limit without embeddings.

With embeddings, time grows roughly with the square of the item count. This was measured on an Apple M1 running other work, with synthetic items in 10 themes of 5 subthemes, 5 terms each, and 256-dimension embeddings:

| Items | Time | Peak memory |
|---|---|---|
| 1,000 | 0.3 s | 21 MB |
| 2,000 | 1.2 s | 34 MB |
| 5,000 | 10 s | 69 MB |

It is built for corpora of thousands of items, the scale `corpus-map` draws, and not for millions.

## What the tests guarantee

The crate has five runtime dependencies (clap, serde, serde_json, safetensors, sha2), and `unsafe` code is forbidden. The tests check four guarantees on every wire format the engine reads:

- the same input gives the same output, byte for byte;
- a territory's identity does not depend on input order;
- the stability bound above holds;
- edge-case corpora still give valid output: an empty corpus, a single item, every item waiting, and items with no terms.

They also check the outputs against the JSON schemas in `contracts/` and against the golden files, and they check that malformed embeddings files are rejected. CI measures line coverage with a floor of 98%, and `cargo deny` checks advisories, licences and sources.
