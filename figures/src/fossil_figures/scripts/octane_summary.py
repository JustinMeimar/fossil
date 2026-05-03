"""Octane aggregate summary: total score + top regressions/improvements.

Renders a horizontal bar chart of per-benchmark deltas between two variants,
sorted by magnitude. Useful for identifying which sub-benchmarks drive
overall score differences.
"""
from __future__ import annotations

import matplotlib.pyplot as plt
import numpy as np
from matplotlib.figure import Figure

from fossil_figures.style import apply_style, palette
from fossil_figures.types import FigureData, Scalar


def render(data: FigureData, baseline: str | None = None) -> Figure:
    apply_style()

    table = data.flat_table()
    columns = data.column_names

    if baseline is None:
        baseline = columns[0]

    if baseline not in table:
        raise ValueError(f"baseline {baseline!r} not in columns: {columns}")

    # Pick first non-baseline column as comparison
    compare = next((c for c in columns if c != baseline), None)
    if compare is None:
        raise ValueError("need at least two columns to compare")

    baseline_metrics = table[baseline]
    compare_metrics = table[compare]

    deltas: list[tuple[str, float, float]] = []
    for metric in sorted(baseline_metrics.keys()):
        b = baseline_metrics.get(metric)
        c = compare_metrics.get(metric)
        if b and c and b.mean != 0:
            pct = ((c.mean - b.mean) / abs(b.mean)) * 100
            err = (c.stddev / abs(b.mean)) * 100
            deltas.append((metric, pct, err))

    deltas.sort(key=lambda t: t[1])

    fig, ax = plt.subplots(figsize=(8, max(4, len(deltas) * 0.35)))

    names = [d[0] for d in deltas]
    pcts = np.array([d[1] for d in deltas])
    errs = np.array([d[2] for d in deltas])
    y = np.arange(len(deltas))

    colors = ["#C73E1D" if p < 0 else "#44AF69" for p in pcts]
    ax.barh(y, pcts, xerr=errs, color=colors, alpha=0.85)
    ax.set_yticks(y)
    ax.set_yticklabels(names)
    ax.axvline(0, color="black", linewidth=0.8)
    ax.set_xlabel(f"% change ({compare} vs {baseline})")
    ax.set_title(f"Octane: {compare} relative to {baseline}")
    fig.tight_layout()
    return fig
