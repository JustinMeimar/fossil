"""Minimal CLI for testing figures outside the fossil pipeline."""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

from fossil_figures.io import load_file, load_stdin
from fossil_figures.style import apply_style


def main() -> None:
    parser = argparse.ArgumentParser(description="Render fossil figures")
    parser.add_argument("script", help="Figure script module to run")
    parser.add_argument("--input", "-i", type=Path, help="JSON input file (default: stdin)")
    parser.add_argument("--output", "-o", type=Path, help="Output file (default: show)")
    args = parser.parse_args()

    apply_style()
    data = load_file(args.input) if args.input else load_stdin()

    # Import and run the named script module
    import importlib

    mod = importlib.import_module(f"fossil_figures.scripts.{args.script}")
    fig = mod.render(data)

    if args.output:
        fig.savefig(args.output)
        print(f"saved: {args.output}", file=sys.stderr)
    else:
        import matplotlib.pyplot as plt

        plt.show()
