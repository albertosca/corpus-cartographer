# Example: a map of hiking trails

`hiking-trails.json` is a small made-up corpus (public domain, CC0) that is not food: fifteen trails, each described by the features it has. It shows the engine laying out and linking a corpus with no embeddings, where two trails are alike only when they share features.

```sh
cargo run -- --input examples/hiking-trails.json --out-dir /tmp/trails-map
```

The output is the same three files as for recipes: `map-spec.v1.json` (the map corpus-map draws), `map-spec.nested.json` and `links.json`.

The file keeps the recipe-era field names of contract v2 (`recipes` for the items, `epicure_term` for a feature), because renaming the contract is left for when the kit moves to its own repository. Read `recipes` as "items" and `epicure_term` as "feature". Terms are compared exactly, so "Ridge" and "ridge" are different features.
