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

    fig, ax = _ensure_axes(ax)

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
    if ylabel:
        ax.set_ylabel(ylabel)
    if title:
        ax.set_title(title)
    ax.legend()
    fig.tight_layout()
    return fig


def delta_bars(
    data: FigureData,
    baseline: str | None = None,
    compare: str | None = None,
    title: str | None = None,
    ax: Axes | None = None,
) -> Figure:
    """Horizontal bars showing per-metric % change between two columns."""
    table = data.flat_table()
    columns = data.column_names

    if baseline is None:
        baseline = columns[0]
    if compare is None:
        compare = next((c for c in columns if c != baseline), None)
    if compare is None:
        raise ValueError("need at least two columns to compare")

    base_m = table[baseline]
    comp_m = table[compare]

    deltas: list[tuple[str, float, float]] = []
    for metric in sorted(base_m.keys()):
        b = base_m.get(metric)
        c = comp_m.get(metric)
        if b and c and b.mean != 0:
            pct = ((c.mean - b.mean) / abs(b.mean)) * 100
            err = (c.stddev / abs(b.mean)) * 100
            deltas.append((metric, pct, err))

    deltas.sort(key=lambda t: t[1])

    fig, ax = _ensure_axes(ax, figsize=(8, max(4, len(deltas) * 0.35)))

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
    if title:
        ax.set_title(title)
    fig.tight_layout()
    return fig


def timeline(
    data: FigureData,
    metric: str,
    title: str | None = None,
    ylabel: str | None = None,
    ax: Axes | None = None,
) -> Figure:
    """Line plot of a single metric across columns (treated as time-ordered)."""
    table = data.flat_table()
    columns = data.column_names
    colors = palette(1)

    means = []
    errs = []
    for col in columns:
        s = table.get(col, {}).get(metric)
        means.append(s.mean if s else float("nan"))
        errs.append(s.stddev if s else 0.0)

    fig, ax = _ensure_axes(ax)
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
    if ylabel:
        ax.set_ylabel(ylabel)
    if title:
        ax.set_title(title)
    fig.tight_layout()
    return fig


def distribution(
    data: FigureData,
    metric: str,
    title: str | None = None,
    ax: Axes | None = None,
) -> Figure:
    """Bar chart of a single metric across columns with error bars."""
    table = data.flat_table()
    columns = data.column_names
    colors = palette(len(columns))

    means = []
    errs = []
    for col in columns:
        s = table.get(col, {}).get(metric)
        means.append(s.mean if s else 0.0)
        errs.append(s.stddev if s else 0.0)

    fig, ax = _ensure_axes(ax)
    x = np.arange(len(columns))
    ax.bar(x, means, yerr=errs, color=colors, alpha=0.8)
    ax.set_xticks(x)
    ax.set_xticklabels(columns, rotation=45, ha="right")
    if title:
        ax.set_title(title)
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
