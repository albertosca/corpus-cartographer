"""Tests for build_arxiv_demo.py: what it refuses, and how.

Run with: python3 -B -m unittest discover -s scripts
"""

import json
import math
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS))
sys.dont_write_bytecode = True

from build_arxiv_demo import build_demo


def map_spec() -> dict:
    """Two papers in one leaf of one family, in corpus-map's demo form."""
    return {
        "version": "1",
        "categories": {},
        "territories": [
            {"id": "family", "name": "Family", "parent": None, "centroid": [0, 0]},
            {"id": "leaf", "name": "Leaf", "parent": "family", "centroid": [0, 0]},
        ],
        "nodes": [
            {"id": "paper-one", "label": "Reed-Solomon list decoding", "x": 0, "y": 0, "territory": "leaf"},
            {"id": "paper-two", "label": "Polar codes", "x": 1, "y": 1, "territory": "leaf"},
        ],
    }


def engine_spec() -> dict:
    """The engine's nested spec for `map_spec`: items numbered from 1, as the export numbers them."""
    return {
        "territories": [
            {"id": "theme:1", "centroid": [5.0, 5.0]},
            {"id": "theme:101", "centroid": [5.0, 5.0]},
        ],
        "nodes": [
            {"id": "item:1", "label": "Reed-Solomon list decoding", "x": 0.0, "y": 0.0, "territory": "theme:101"},
            {"id": "item:2", "label": "Polar codes", "x": 10.0, "y": 4.0, "territory": "theme:101"},
        ],
        "edges": [{"source": "item:1", "target": "item:2", "weight": 0.5}],
    }


class BuildDemoTest(unittest.TestCase):
    def assertRefused(self, engine: dict, message: str) -> None:
        with self.assertRaises(SystemExit) as raised:
            build_demo(map_spec(), engine)
        self.assertIn(message, str(raised.exception))

    def test_writes_positions_and_links(self) -> None:
        demo = build_demo(map_spec(), engine_spec())
        self.assertEqual([[node["x"], node["y"]] for node in demo["nodes"]], [[0.0, 0.0], [1000.0, 400.0]])
        self.assertEqual(demo["edges"], [[0, 1, 50]])

    def test_refuses_a_paper_whose_id_is_not_its_position(self) -> None:
        engine = engine_spec()
        engine["nodes"][0]["id"] = "item:7"
        self.assertRefused(engine, "does not list the same papers in the same order")

    def test_refuses_a_coordinate_that_is_not_a_number(self) -> None:
        engine = engine_spec()
        engine["nodes"][1]["y"] = math.nan
        self.assertRefused(engine, "not a finite position")

    def test_refuses_a_centroid_that_is_missing(self) -> None:
        engine = engine_spec()
        engine["territories"][1]["centroid"] = None
        self.assertRefused(engine, "not a finite position")

    def test_refuses_a_self_link(self) -> None:
        engine = engine_spec()
        engine["edges"] = [{"source": "item:1", "target": "item:1", "weight": 0.5}]
        self.assertRefused(engine, "links a paper to itself")

    def test_refuses_a_link_listed_twice(self) -> None:
        engine = engine_spec()
        engine["edges"] = [
            {"source": "item:1", "target": "item:2", "weight": 0.5},
            {"source": "item:2", "target": "item:1", "weight": 0.5},
        ]
        self.assertRefused(engine, "listed twice")

    def test_refuses_a_weight_the_demo_cannot_hold(self) -> None:
        for weight in (0.004, 0.0, -0.5, 1.2, math.nan):
            with self.subTest(weight=weight):
                engine = engine_spec()
                engine["edges"][0]["weight"] = weight
                self.assertRefused(engine, "weight")


class CommandLineTest(unittest.TestCase):
    def run_script(self, *arguments: str) -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, "-I", str(SCRIPTS / "build_arxiv_demo.py"), *arguments],
            capture_output=True,
            text=True,
            check=False,
        )

    def test_help_prints_the_usage_and_succeeds(self) -> None:
        result = self.run_script("--help")
        self.assertEqual(result.returncode, 0)
        self.assertIn("Usage:", result.stdout)

    def test_a_wrong_argument_count_prints_the_usage_and_fails(self) -> None:
        result = self.run_script("only-one.json")
        self.assertEqual(result.returncode, 1)
        self.assertIn("Usage:", result.stderr)

    def test_unreadable_input_is_a_refusal_not_a_traceback(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            (folder / "map.json").write_text(json.dumps(map_spec()))
            (folder / "truncated.json").write_text(json.dumps(engine_spec())[:40])
            (folder / "no-nodes.json").write_text(json.dumps({"territories": [], "edges": []}))
            for engine_name, message in (("truncated.json", "is not valid JSON"), ("no-nodes.json", "has no 'nodes'")):
                with self.subTest(engine=engine_name):
                    output = folder / "out.json"
                    result = self.run_script(str(folder / "map.json"), str(folder / engine_name), str(output))
                    self.assertEqual(result.returncode, 1)
                    self.assertIn(message, result.stderr)
                    self.assertNotIn("Traceback", result.stderr)
                    self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
