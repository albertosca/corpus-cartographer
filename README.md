🇺🇸 [English](README.md) · 🇧🇷 [Português](README.pt.md)

# corpus-cartographer

A command-line engine that lays out a corpus as a map: it reads an `engine-export` JSON document (themes, items and the terms each item has), optionally a directory of term embeddings, and writes the links between items plus two map specs.

```sh
cargo run -- --input examples/hiking-trails.json --out-dir /tmp/trails
```

Work in progress: the contract is being renamed to generic words (v3) while v2 stays readable.
