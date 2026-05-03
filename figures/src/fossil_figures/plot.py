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
    """Bar chart comparing metrics across columns (variants/records).

    If normalize_to is given, values are shown as ratios relative to that column.
    """
    table = data.flat_table()
    columns = data.column_names
    all_metrics = metrics or data.metric_names()

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

    fig: Figure | None = None
    if ax is None:
        fig, ax = plt.subplots()
    else:
        fig = ax.get_figure()

    n_cols = len(columns)
    n_metrics = len(all_metrics)
    x = np.arange(n_metrics)
    width = 0.8 / n_cols
    colors = palette(n_cols)

    for i, col in enumerate(columns):
        means = []
        errs = []
        for m in all_metrics:
            s = table.get(col, {}).get(m)
            means.append(s.mean if s else 0.0)
            errs.append(s.stddev if s else 0.0)
        offset = (i - n_cols / 2 + 0.5) * width
        ax.bar(x + offset, means, width, yerr=errs, label=col, color=colors[i])

    ax.set_xticks(x)
    ax.set_xticklabels(all_metrics, rotation=45, ha="right")
    if ylabel:
        ax.set_ylabel(ylabel)
    if title:
        ax.set_title(title)
    ax.legend()

    assert fig is not None
    fig.tight_layout()
    return fig


def timeline(
    records: list[FigureData],
    metric: str,
    labels: Sequence[str] | None = None,
    title: str | None = None,
    ylabel: str | None = None,
    ax: Axes | None = None,
) -> Figure:
    """Line plot of a single metric across sequential records (time axis)."""
    fig: Figure | None = None
    if ax is None:
        fig, ax = plt.subplots()
    else:
        fig = ax.get_figure()

    all_columns: set[str] = set()
    for rd in records:
        all_columns.update(rd.column_names)

    colors = palette(len(all_columns))
    x_labels = labels or [str(i) for i in range(len(records))]
    x = np.arange(len(records))

    for ci, col in enumerate(sorted(all_columns)):
        means = []
        errs = []
        for rd in records:
            table = rd.flat_table()
            s = table.get(col, {}).get(metric)
            means.append(s.mean if s else float("nan"))
            errs.append(s.stddev if s else 0.0)
        ax.errorbar(x, means, yerr=errs, label=col, color=colors[ci], marker="o", markersize=4)

    ax.set_xticks(x)
    ax.set_xticklabels(x_labels, rotation=45, ha="right")
    if ylabel:
        ax.set_ylabel(ylabel)
    if title:
        ax.set_title(title)
    ax.legend()

    assert fig is not None
    fig.tight_layout()
    return fig


def distribution(
    data: FigureData,
    metric: str,
    title: str | None = None,
    ax: Axes | None = None,
) -> Figure:
    """Visualize metric distributions across columns using error bands.

    Since fossil aggregates to mean/stddev, we show point + error bar per column.
    """
    fig: Figure | None = None
    if ax is None:
        fig, ax = plt.subplots()
    else:
        fig = ax.get_figure()

    table = data.flat_table()
    columns = data.column_names
    colors = palette(len(columns))

    means = []
    errs = []
    for col in columns:
        s = table.get(col, {}).get(metric)
        means.append(s.mean if s else 0.0)
        errs.append(s.stddev if s else 0.0)

    x = np.arange(len(columns))
    ax.bar(x, means, yerr=errs, color=colors, alpha=0.8)
    ax.set_xticks(x)
    ax.set_xticklabels(columns, rotation=45, ha="right")
    if title:
        ax.set_title(title)

    assert fig is not None
    fig.tight_layout()
    return fig
