"""Tests for coverage_gate.py. Run with `python3 -m unittest discover .github/scripts`."""

import json
import tempfile
import unittest
from pathlib import Path

import coverage_gate


def report(root, files):
    """Build a minimal `cargo llvm-cov --json --summary-only` report."""
    return {
        "data": [
            {
                "files": [
                    {
                        "filename": str(root / name),
                        "summary": {"lines": {"count": count, "covered": covered}},
                    }
                    for name, count, covered in files
                ]
            }
        ]
    }


class GateTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        for crate in ("couchfin-api", "couchfin-core"):
            (self.root / "crates" / crate / "src").mkdir(parents=True)

    def tearDown(self):
        self.tmp.cleanup()

    def run_gate(self, files, crates=("couchfin-api", "couchfin-core")):
        path = self.root / "cov.json"
        path.write_text(json.dumps(report(self.root, files)))
        return coverage_gate.main(
            [str(path), "--root", str(self.root), "--min", "80", *crates]
        )

    def test_passes_at_threshold(self):
        code = self.run_gate(
            [
                ("crates/couchfin-api/src/lib.rs", 10, 8),
                ("crates/couchfin-core/src/lib.rs", 100, 95),
            ]
        )
        self.assertEqual(code, 0)

    def test_fails_when_one_crate_is_below(self):
        code = self.run_gate(
            [
                ("crates/couchfin-api/src/lib.rs", 100, 100),
                ("crates/couchfin-core/src/lib.rs", 100, 79),
            ]
        )
        self.assertEqual(code, 1)

    def test_sums_files_per_crate(self):
        # 50/50 + 30/50 = 80/100 for api; files of other crates don't count.
        code = self.run_gate(
            [
                ("crates/couchfin-api/src/a.rs", 50, 50),
                ("crates/couchfin-api/src/b.rs", 50, 30),
                ("crates/couchfin-core/src/lib.rs", 10, 10),
                ("crates/couchfin-ui/src/lib.rs", 100, 0),
            ]
        )
        self.assertEqual(code, 0)

    def test_crate_name_prefix_does_not_match_other_crate(self):
        (self.root / "crates" / "couchfin-api-extra" / "src").mkdir(parents=True)
        code = self.run_gate(
            [
                ("crates/couchfin-api/src/lib.rs", 10, 10),
                ("crates/couchfin-api-extra/src/lib.rs", 100, 0),
                ("crates/couchfin-core/src/lib.rs", 10, 10),
            ]
        )
        self.assertEqual(code, 0)

    def test_crate_without_executable_lines_is_skipped_not_failed(self):
        code = self.run_gate([("crates/couchfin-core/src/lib.rs", 10, 9)])
        self.assertEqual(code, 0)

    def test_unknown_crate_fails(self):
        code = self.run_gate(
            [("crates/couchfin-api/src/lib.rs", 10, 10)],
            crates=("couchfin-api", "couchfin-typo"),
        )
        self.assertEqual(code, 1)


if __name__ == "__main__":
    unittest.main()
