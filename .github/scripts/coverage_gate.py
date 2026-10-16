"""Per-crate line coverage gate for CI.

Reads the JSON summary from `cargo llvm-cov report --json --summary-only` and
fails if any named crate's line coverage (files under `crates/<name>/src/`) is
below the threshold. cargo-llvm-cov's own `--fail-under-lines` only checks the
workspace total, which a well-tested crate could hide a poorly tested one in.

A crate with no executable lines yet (an empty library) is skipped with a
warning instead of failing on 0% or passing on a meaningless 100%. A crate
directory that doesn't exist fails, so a typo can't turn the gate off.
See docs/dev-environment.md ("Coverage").
"""

import argparse
import json
import os
import sys
from pathlib import Path


def crate_lines(report, crate_src):
    """Sum (count, covered) line totals for the files under `crate_src`."""
    count = covered = 0
    for data in report["data"]:
        for file in data["files"]:
            if Path(file["filename"]).resolve().is_relative_to(crate_src):
                lines = file["summary"]["lines"]
                count += lines["count"]
                covered += lines["covered"]
    return count, covered


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("report", help="cargo llvm-cov JSON summary")
    parser.add_argument("crates", nargs="+", help="crate directory names under crates/")
    parser.add_argument("--min", type=float, required=True, help="minimum line coverage, percent")
    parser.add_argument("--root", default=".", help="workspace root")
    args = parser.parse_args(argv)

    report = json.loads(Path(args.report).read_text())
    root = Path(args.root).resolve()
    failed = False
    rows = []

    for crate in args.crates:
        crate_src = root / "crates" / crate / "src"
        if not crate_src.is_dir():
            print(f"::error title=coverage::{crate}: no such crate ({crate_src})")
            rows.append((crate, "-", "missing"))
            failed = True
            continue

        count, covered = crate_lines(report, crate_src)
        if count == 0:
            print(
                f"::warning title=coverage::{crate} has no executable lines yet, "
                f"so the {args.min:g}% gate is not applied to it"
            )
            rows.append((crate, "0/0", "skipped (no code yet)"))
            continue

        percent = 100.0 * covered / count
        ok = percent >= args.min
        if not ok:
            print(
                f"::error title=coverage::{crate} line coverage {percent:.2f}% "
                f"is below {args.min:g}%"
            )
            failed = True
        rows.append((crate, f"{covered}/{count} ({percent:.2f}%)", "ok" if ok else "FAIL"))

    table = [f"| Crate | Lines covered | Result (min {args.min:g}%) |", "|---|---|---|"]
    table += [f"| `{crate}` | {lines} | {result} |" for crate, lines, result in rows]
    print("\n".join(table))
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as out:
            out.write("## Coverage\n\n" + "\n".join(table) + "\n")

    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
