#!/usr/bin/env python3
"""Fossil figure script: Octane score timeline.

Line plot tracking a metric across time-ordered columns (records).
Defaults to "total" score; override with FOSSIL_METRIC env var.

Usage in fossil.toml:
    [figures.octane-timeline]
    analysis = "octane"
    script = "figures/bin/octane_timeline.py"
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "src"))

import matplotlib

matplotlib.use("Agg")

from fossil_figures import apply_style, load_stdin
from fossil_figures.scripts.octane_timeline import render


def main() -> None:
    apply_style()
    data = load_stdin()

    fossil_dir = Path(os.environ.get("FOSSIL_DIR", "."))
    figure_name = os.environ.get("FOSSIL_FIGURE_NAME", "timeline")
    metric = os.environ.get("FOSSIL_METRIC", "total")

    fig = render(data, metric=metric)

    out = fossil_dir / "figures" / f"{figure_name}.pdf"
    out.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(out)
    print(f"wrote {out}", file=sys.stderr)


if __name__ == "__main__":
    main()
