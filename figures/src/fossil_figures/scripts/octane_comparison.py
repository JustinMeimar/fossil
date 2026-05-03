"""Compare Octane sub-benchmark scores across variants.

Renders a grouped bar chart with one bar per variant, grouped by sub-benchmark.
Suitable for answering: "how does disabling Ion affect each sub-benchmark?"
"""
from __future__ import annotations

import matplotlib.pyplot as plt
import numpy as np
from matplotlib.figure import Figure

from fossil_figures.style import apply_style, palette
from fossil_figures.types import FigureData, Scalar


def render(data: FigureData, normalize_to: str | None = None) -> Figure:
    apply_style()

    table = data.flat_table()
    columns = data.column_names
    all_metrics = data.metric_names()

    if normalize_to and normalize_to in table:
        baseline = table[normalize_to]
        normalized: dict[str, dict[str, Scalar]] = {}
        for col, col_metrics in table.items():
            normalized[col] = {}
            for m in all_metrics:
                if m in col_metrics and m in baseline:
                    normalized[col][m] = col_metrics[m].normalized_to(baseline[m])
        table = normalized

    fig, ax = plt.subplots(figsize=(10, 5))

    n_cols = len(columns)
    n_metrics = len(all_metrics)
    x = np.arange(n_metrics)
    width = 0.8 / max(n_cols, 1)
    colors = palette(n_cols)

    for i, col in enumerate(columns):
        means = [table.get(col, {}).get(m, Scalar(0, 0)).mean for m in all_metrics]
        errs = [table.get(col, {}).get(m, Scalar(0, 0)).stddev for m in all_metrics]
        offset = (i - n_cols / 2 + 0.5) * width
        ax.bar(x + offset, means, width, yerr=errs, label=col, color=colors[i])

    ax.set_xticks(x)
    ax.set_xticklabels(all_metrics, rotation=45, ha="right")
    ax.set_ylabel("Score" if not normalize_to else f"Relative to {normalize_to}")
    ax.set_title("Octane Sub-benchmark Scores")
    ax.legend(loc="upper right")
    fig.tight_layout()
    return fig
