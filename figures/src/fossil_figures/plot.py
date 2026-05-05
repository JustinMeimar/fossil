from __future__ import annotations

from typing import Sequence

import matplotlib.pyplot as plt
import numpy as np
from matplotlib.axes import Axes
from matplotlib.figure import Figure

from fossil_figures.style import palette
from fossil_figures.types import FigureData, Scalar


def comparison_bar(
    data: FigureData,
    metrics: Sequence[str] | None = None,
    normalize_to: str | None = None,
    title: str | None = None,
    ylabel: str | None = None,
    ax: Axes | None = None,
) -> Figure:
    """Grouped bar chart comparing metrics across columns."""
    table = data.flat_table()
    columns = data.column_names
    all_metrics = list(metrics) if metrics else data.metric_names()

    n_cols = len(columns)
    n_metrics = len(all_metrics)

    if normalize_to and normalize_to in table:
        baseline = table[normalize_to]
        resolved: dict[str, dict[str, Scalar]] = {}
        for col, col_metrics in table.items():
            resolved[col] = {}
            for m in all_metrics:
                if m in col_metrics and m in baseline:
                    resolved[col][m] = col_metrics[m].normalized_to(baseline[m])
        table = resolved
        if ylabel is None:
            ylabel = f"Relative to {normalize_to}"

    fig, ax = _ensure_axes(ax, figsize=(max(10, n_metrics * 0.7), 5))

    x = np.arange(n_metrics)
    group_w = 0.8
    width = group_w / max(n_cols, 1)
    colors = palette(n_cols)

    for i, col in enumerate(columns):
        means = [table.get(col, {}).get(m, Scalar(0, 0)).mean for m in all_metrics]
        errs = [table.get(col, {}).get(m, Scalar(0, 0)).stddev for m in all_metrics]
        offset = (i - n_cols / 2 + 0.5) * width
        ax.bar(
            x + offset, means, width, yerr=errs,
            label=col, color=colors[i], edgecolor="none",
        )

    ax.set_xticks(x)
    ax.set_xticklabels(all_metrics, rotation=45, ha="right")
    if ylabel:
        ax.set_ylabel(ylabel)
    if title:
        ax.set_title(title)
    ax.legend()
    fig.tight_layout()
    return fig


def _ensure_axes(
    ax: Axes | None, figsize: tuple[float, float] | None = None,
) -> tuple[Figure, Axes]:
    if ax is not None:
        fig = ax.get_figure()
        assert fig is not None
        return fig, ax
    return plt.subplots(figsize=figsize)
