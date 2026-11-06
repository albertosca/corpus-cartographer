# Example: a map of hiking trails

`hiking-trails.json` is a small made-up corpus (public domain, CC0) that is not food: fifteen trails, each described by the features it has. It shows the engine laying out and linking a corpus with no embeddings, where two trails are alike only when they share features.

```sh
cargo run -- --input examples/hiking-trails.json --out-dir /tmp/trails-map
```

The output is three files: `map-spec.v1.json` (the map `corpus-map` draws), `map-spec.nested.json` and `links.json`.

The file is a contract v3 `engine-export`: each trail is an item, each feature is a term (`label` is what the producer calls it, `term` is the word compared). Terms are compared exactly, so "Ridge" and "ridge" are different features.

`hiking-trails.v2.json` is the same corpus in the older v2 shape. The engine still reads it and answers in v2; a test checks that both files give the same links.
