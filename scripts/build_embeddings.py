"""Build an embeddings directory for corpus-cartographer from an export.

Usage: python scripts/build_embeddings.py EXPORT.json MODEL OUTPUT_DIRECTORY

Encodes every term the export names (theme terms and item terms; a null
term is skipped) with a Model2Vec static model and writes vocab.json and
embeddings.safetensors, the two files --embeddings-dir reads.
Requires: pip install model2vec
"""

import json
import sys
from pathlib import Path

from model2vec import StaticModel
from safetensors.numpy import save_file


def export_terms(export: dict) -> list[str]:
    theme_terms = {term for theme in export["themes"] for term in theme["terms"]}
    item_terms = {entry["term"] for item in export["items"] for entry in item["terms"] if entry["term"] is not None}
    return sorted(theme_terms | item_terms)


def main() -> None:
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    export_path, model_name, output_directory = Path(sys.argv[1]), sys.argv[2], Path(sys.argv[3])
    terms = export_terms(json.loads(export_path.read_text()))
    if not terms:
        sys.exit(f"{export_path} names no terms, so there is nothing to encode")
    vectors = StaticModel.from_pretrained(model_name).encode(terms).astype("float32")
    output_directory.mkdir(parents=True, exist_ok=True)
    vocabulary = {term: index for index, term in enumerate(terms)}
    (output_directory / "vocab.json").write_text(json.dumps(vocabulary, ensure_ascii=False))
    save_file({"embeddings": vectors}, output_directory / "embeddings.safetensors")
    print(f"{len(terms)} terms, {vectors.shape[1]} dimensions, written to {output_directory}")


if __name__ == "__main__":
    main()
