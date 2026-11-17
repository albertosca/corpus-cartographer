"""Build a corpus-cartographer export from the arXiv sample that corpus-map ships.

Usage: python scripts/build_arxiv_export.py ARXIV_MAP_SPEC.json OUTPUT_EXPORT.json

ARXIV_MAP_SPEC.json is corpus-map's demo/corpora/arxiv.json: 8,000
computer-science papers from arXiv (metadata under CC0 1.0), each with a
title and the territory its abstract landed in. The territories become the
themes (families as top-level themes, leaves as subthemes), and the terms of
each paper are the words of its title, minus stop words. Only the standard
library is needed.

Thank you to arXiv for use of its open access interoperability.
"""

import json
import re
import sys
from pathlib import Path

STOP_WORDS = frozenset(
    """
    a about above across after against all along also among an and any are as at be been being between beyond
    both but by can could do does doing during each either for from further has have how if in into is it its
    just more most much new no non nor not of off on one only or other our out over own per same should so some
    such than that the their them then there these they this those through to too toward towards under until
    up upon us use used uses using via vs was we were what when where whether which while who whom why will
    with within without would you your
    """.split()
)

WORD = re.compile(r"[A-Za-z][A-Za-z0-9]*(?:[+'][A-Za-z0-9]+)*")


def title_terms(title: str) -> list[dict]:
    """Each distinct word of a title, in order: kept when it is an acronym or at least three letters long."""
    terms: list[dict] = []
    seen: set[str] = set()
    for word in WORD.findall(title.replace("-", " ")):
        term = word.lower()
        is_acronym = len(word) >= 2 and word.isupper()
        if term in STOP_WORDS or term in seen or (len(word) < 3 and not is_acronym):
            continue
        seen.add(term)
        terms.append({"label": word, "term": term})
    return terms


def build_export(map_spec: dict) -> dict:
    families = [territory for territory in map_spec["territories"] if territory.get("parent") is None]
    leaves = [territory for territory in map_spec["territories"] if territory.get("parent") is not None]
    family_ids = {family["id"]: position for position, family in enumerate(families, start=1)}
    leaf_ids = {leaf["id"]: 100 + position for position, leaf in enumerate(leaves, start=1)}
    themes = [{"id": family_ids[family["id"]], "parent_id": None, "name": family["name"], "terms": []} for family in families]
    themes += [
        {"id": leaf_ids[leaf["id"]], "parent_id": family_ids[leaf["parent"]], "name": leaf["name"], "terms": []}
        for leaf in leaves
    ]
    items = [
        {
            "id": position,
            "title": node["label"],
            "subtheme_id": leaf_ids[node["territory"]],
            "status": "assigned",
            "badges": [],
            "terms": title_terms(node["label"]),
        }
        for position, node in enumerate(map_spec["nodes"], start=1)
    ]
    return {
        "schema": "corpus-cartographer/engine-export",
        "version": 3,
        "taxonomy_version": 1,
        "themes": themes,
        "items": items,
        "previous": None,
        "rejected_pairs": [],
    }


def main() -> None:
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    map_spec_path, output_path = Path(sys.argv[1]), Path(sys.argv[2])
    export = build_export(json.loads(map_spec_path.read_text()))
    output_path.write_text(json.dumps(export, ensure_ascii=False))
    term_count = len({entry["term"] for item in export["items"] for entry in item["terms"]})
    print(f"{len(export['items'])} items, {len(export['themes'])} themes, {term_count} distinct terms, written to {output_path}")


if __name__ == "__main__":
    main()
