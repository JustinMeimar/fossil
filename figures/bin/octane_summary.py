#!/usr/bin/env python3
"""Fossil figure script: Octane delta summary.

Horizontal bar chart showing per-benchmark % change between two variants.
First column is treated as baseline unless FOSSIL_BASELINE is set.

Usage in fossil.toml:
    [figures.octane-summary]
    analysis = "octane"
    script = "figures/bin/octane_summary.py"
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "src"))

import matplotlib

matplotlib.use("Agg")

from fossil_figures import apply_style, load_stdin
from fossil_figures.scripts.octane_summary import render


def main() -> None:
    apply_style()
    data = load_stdin()

    fossil_dir = Path(os.environ.get("FOSSIL_DIR", "."))
    figure_name = os.environ.get("FOSSIL_FIGURE_NAME", "summary")
    baseline = os.environ.get("FOSSIL_BASELINE")

    fig = render(data, baseline=baseline)

    out = fossil_dir / "figures" / f"{figure_name}.pdf"
    out.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(out)
    print(f"wrote {out}", file=sys.stderr)


if __name__ == "__main__":
    main()
