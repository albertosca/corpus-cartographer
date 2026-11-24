"""Write a corpus-cartographer map of the arXiv sample back as corpus-map's demo payload.

Usage: python scripts/build_arxiv_demo.py ARXIV_MAP_SPEC.json ENGINE_NESTED_SPEC.json OUTPUT.json

The return trip of build_arxiv_export.py. ARXIV_MAP_SPEC.json is corpus-map's
demo/corpora/arxiv.json, the file the export was built from;
ENGINE_NESTED_SPEC.json is the engine's map-spec.nested.json for that export.
The output keeps the papers' ids, titles and categories and the territories'
names, and takes the engine's positions, centroids and links: positions are
scaled into 0..1000 across, the aspect kept, and each link is written as
[source index, target index, weight x 100]. It is written in the compact form
corpus-map's demo ships, which is how the live demo's arXiv corpus was made.
Only the standard library is needed.

Thank you to arXiv for use of its open access interoperability.
"""

import json
import math
import sys
from pathlib import Path

# The id rule lives in the export script beside this one; `python -I` leaves the script directory off the path,
# and importing it should not leave a __pycache__ in the repository.
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.dont_write_bytecode = True

from build_arxiv_export import theme_ids


def is_number(value: object) -> bool:
    """A finite int or float; a bool is not one, though Python counts it as an int."""
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value)


def build_demo(map_spec: dict, engine_spec: dict) -> dict:
    """The demo payload: `map_spec` with the engine's positions, centroids and links. Raises SystemExit on a mismatch."""
    nodes = map_spec["nodes"]
    engine_nodes = engine_spec["nodes"]
    # The export numbers papers from 1 in this spec's order, and the engine names each one item:<that number>.
    if len(nodes) != len(engine_nodes) or any(
        node["label"] != engine_node["label"] or engine_node["id"] != f"item:{position}"
        for position, (node, engine_node) in enumerate(zip(nodes, engine_nodes), start=1)
    ):
        raise SystemExit("the engine spec does not list the same papers in the same order")
    family_ids, leaf_ids = theme_ids(map_spec)
    engine_id_of = {territory: f"theme:{number}" for territory, number in {**family_ids, **leaf_ids}.items()}
    territory_of = {engine_id: territory for territory, engine_id in engine_id_of.items()}
    for node, engine_node in zip(nodes, engine_nodes):
        if territory_of.get(engine_node.get("territory")) != node["territory"]:
            raise SystemExit(f"paper {node['id']} sits in another territory in the engine spec")
    engine_territories = {territory["id"]: territory for territory in engine_spec["territories"]}
    missing = [
        territory["id"]
        for territory in map_spec["territories"]
        if engine_id_of[territory["id"]] not in engine_territories
    ]
    if missing:
        raise SystemExit(f"the engine spec has no territory for {', '.join(missing)}")

    positions = [[node["x"], node["y"]] for node in engine_nodes]
    positions += [
        engine_territories[engine_id_of[territory["id"]]].get("centroid") for territory in map_spec["territories"]
    ]
    if not all(
        isinstance(position, list) and len(position) == 2 and all(is_number(value) for value in position)
        for position in positions
    ):
        raise SystemExit("the engine spec has a paper or territory that is not a finite position")

    minimum_horizontal = min(node["x"] for node in engine_nodes)
    maximum_horizontal = max(node["x"] for node in engine_nodes)
    minimum_vertical = min(node["y"] for node in engine_nodes)
    if maximum_horizontal <= minimum_horizontal:
        raise SystemExit("the engine spec places every paper at the same horizontal position")
    scale = 1000.0 / (maximum_horizontal - minimum_horizontal)

    def place(horizontal: float, vertical: float) -> list[float]:
        return [round((horizontal - minimum_horizontal) * scale, 3), round((vertical - minimum_vertical) * scale, 3)]

    demo = json.loads(json.dumps(map_spec))
    for node, engine_node in zip(demo["nodes"], engine_nodes):
        node["x"], node["y"] = place(engine_node["x"], engine_node["y"])
    for territory in demo["territories"]:
        territory["centroid"] = place(*engine_territories[engine_id_of[territory["id"]]]["centroid"])
    index_of = {engine_node["id"]: index for index, engine_node in enumerate(engine_nodes)}
    edges = engine_spec.get("edges", [])
    unknown = [edge for edge in edges if edge["source"] not in index_of or edge["target"] not in index_of]
    if unknown:
        raise SystemExit(f"{len(unknown)} engine links name a paper the spec does not list")
    demo["edges"] = []
    pairs = set()
    for edge in edges:
        source, target = index_of[edge["source"]], index_of[edge["target"]]
        if source == target:
            raise SystemExit(f"an engine link links a paper to itself ({edge['source']})")
        if frozenset((source, target)) in pairs:
            raise SystemExit(f"the engine link between {edge['source']} and {edge['target']} is listed twice")
        pairs.add(frozenset((source, target)))
        # corpus-map's demo holds a weight as a whole number in 1..100.
        weight = edge["weight"]
        if not (is_number(weight) and 0 < weight <= 1 and round(weight * 100) >= 1):
            raise SystemExit(
                f"the engine link {edge['source']}-{edge['target']} has weight {weight!r}, outside (0.005, 1]"
            )
        demo["edges"].append([source, target, round(weight * 100)])
    return demo


def read_spec(path: Path, keys: tuple[str, ...]) -> dict:
    """The JSON object at `path`, with each of `keys`. Raises SystemExit on a file that is not one."""
    try:
        spec = json.loads(path.read_text())
    except OSError as error:
        raise SystemExit(f"{path} cannot be read: {error.strerror}") from None
    except json.JSONDecodeError as error:
        raise SystemExit(f"{path} is not valid JSON ({error.msg}, line {error.lineno})") from None
    for key in keys:
        if not isinstance(spec, dict) or key not in spec:
            raise SystemExit(f"{path} has no '{key}'")
    return spec


def main() -> None:
    if sys.argv[1:] in (["--help"], ["-h"]):
        print(__doc__)
        return
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    map_spec_path, engine_path, output_path = (Path(argument) for argument in sys.argv[1:4])
    map_spec, engine_spec = (
        read_spec(map_spec_path, ("nodes", "territories")),
        read_spec(engine_path, ("nodes", "territories")),
    )
    try:
        demo = build_demo(map_spec, engine_spec)
        payload = json.dumps(demo, separators=(",", ":"), ensure_ascii=False, allow_nan=False)
    except (KeyError, TypeError, ValueError, OverflowError) as error:
        raise SystemExit(
            f"the specs are not the shape this converter reads ({type(error).__name__}: {error})"
        ) from None
    try:
        output_path.write_text(payload)
    except OSError as error:
        raise SystemExit(f"{output_path} cannot be written: {error.strerror}") from None
    print(
        f"{len(demo['nodes'])} papers, {len(demo['territories'])} territories, {len(demo['edges'])} links, written to {output_path}"
    )


if __name__ == "__main__":
    main()
