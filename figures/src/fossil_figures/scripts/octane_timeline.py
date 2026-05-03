"""Octane score over time: track a single metric across sequential records.

This script is designed to be called multiple times (once per record batch)
or with multi-column data representing sequential snapshots.
"""
from __future__ import annotations

import matplotlib.pyplot as plt
import numpy as np
from matplotlib.figure import Figure

from fossil_figures.style import apply_style, palette
from fossil_figures.types import FigureData


def render(data: FigureData, metric: str = "total") -> Figure:
    """Render timeline for a specific metric across columns (treated as time-ordered)."""
    apply_style()

    table = data.flat_table()
    columns = data.column_names
    colors = palette(1)

    means = []
    errs = []
    for col in columns:
        s = table.get(col, {}).get(metric)
        means.append(s.mean if s else float("nan"))
        errs.append(s.stddev if s else 0.0)

    fig, ax = plt.subplots(figsize=(8, 4))
    x = np.arange(len(columns))

    ax.errorbar(x, means, yerr=errs, color=colors[0], marker="o", markersize=5, linewidth=1.5)
    ax.fill_between(
        x,
        np.array(means) - np.array(errs),
        np.array(means) + np.array(errs),
        alpha=0.15,
        color=colors[0],
    )

    ax.set_xticks(x)
    ax.set_xticklabels(columns, rotation=45, ha="right")
    ax.set_ylabel(metric)
    ax.set_title(f"Octane: {metric} over time")
    fig.tight_layout()
    return fig
