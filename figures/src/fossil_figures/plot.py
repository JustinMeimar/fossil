from __future__ import annotations

from typing import Sequence

import matplotlib.patches as mpatches
import matplotlib.pyplot as plt
import numpy as np
from matplotlib.axes import Axes
from matplotlib.figure import Figure

from fossil_figures.style import palette
from fossil_figures.types import FigureData, Scalar

Table = dict[str, dict[str, Scalar]]


def _resolve_table(
    data: FigureData,
    metrics: Sequence[str] | None,
    normalize_to: str | None,
) -> tuple[Table, list[str], str | None]:
    """Flatten data, apply normalization, return (table, metric_names, ylabel)."""
    table = data.flat_table()
    all_metrics = list(metrics) if metrics else data.metric_names()
    ylabel = None

    if normalize_to and normalize_to in table:
        baseline = table[normalize_to]
        resolved: Table = {}
        for col, col_metrics in table.items():
            resolved[col] = {}
            for m in all_metrics:
                if m in col_metrics and m in baseline:
                    resolved[col][m] = col_metrics[m].normalized_to(baseline[m])
        table = resolved
        ylabel = f"Relative to {normalize_to}"

    return table, all_metrics, ylabel


def comparison_bar(
    data: FigureData,
    metrics: Sequence[str] | None = None,
    normalize_to: str | None = None,
    title: str | None = None,
    ylabel: str | None = None,
    ax: Axes | None = None,
) -> Figure:
    """Grouped bar chart comparing metrics across columns."""
    table, all_metrics, norm_label = _resolve_table(data, metrics, normalize_to)
    if ylabel is None:
        ylabel = norm_label
    columns = data.column_names
    n_cols = len(columns)
    n_metrics = len(all_metrics)

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


def violin(
    data: FigureData,
    metrics: Sequence[str] | None = None,
    normalize_to: str | None = None,
    title: str | None = None,
    ylabel: str | None = None,
    samples: int = 200,
    ax: Axes | None = None,
) -> Figure:
    """Violin plot showing metric distributions across columns.

    Generates kernel-density violins from mean+stddev summary statistics.
    Each column becomes a violin body at every metric position.
    """
    table, all_metrics, norm_label = _resolve_table(data, metrics, normalize_to)
    if ylabel is None:
        ylabel = norm_label
    columns = data.column_names
    n_cols = len(columns)
    n_metrics = len(all_metrics)

    fig, ax = _ensure_axes(ax, figsize=(max(10, n_metrics * 0.8), 5))
    colors = palette(n_cols)
    group_w = 0.8
    width = group_w / max(n_cols, 1)
    rng = np.random.default_rng(42)

    for i, col in enumerate(columns):
        positions = []
        violins_data = []
        for j, m in enumerate(all_metrics):
            s = table.get(col, {}).get(m)
            pos = j + (i - n_cols / 2 + 0.5) * width
            positions.append(pos)
            if s is None or s.stddev == 0:
                violins_data.append(np.full(samples, s.mean if s else 0.0))
            else:
                violins_data.append(rng.normal(s.mean, s.stddev, samples))

        parts = ax.violinplot(
            violins_data,
            positions=positions,
            widths=width * 0.9,
            showmeans=True,
            showextrema=False,
        )
        for body in parts["bodies"]:
            body.set_facecolor(colors[i])
            body.set_alpha(0.7)
        parts["cmeans"].set_color(colors[i])
        parts["cmeans"].set_linewidth(1.5)

    ax.set_xticks(range(n_metrics))
    ax.set_xticklabels(all_metrics, rotation=45, ha="right")
    if ylabel:
        ax.set_ylabel(ylabel)
    if title:
        ax.set_title(title)

    legend_patches = [
        mpatches.Patch(color=colors[i], alpha=0.7, label=col)
        for i, col in enumerate(columns)
    ]
    ax.legend(handles=legend_patches)
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
