from __future__ import annotations

from typing import Callable, Sequence

import matplotlib.patches as mpatches
import matplotlib.pyplot as plt
import numpy as np
from matplotlib.axes import Axes
from matplotlib.figure import Figure

from fossil_figures.style import palette
from fossil_figures.types import FigureData, Scalar

Table = dict[str, dict[str, Scalar]]
Panel = Callable[..., object]


def _resolve_table(
    data: FigureData,
    metrics: Sequence[str] | None,
    normalize_to: str | None,
) -> tuple[Table, list[str], str | None]:
    """Flatten data, apply normalization, return (table, metric_names, label)."""
    table = data.flat_table()
    all_metrics = list(metrics) if metrics else data.metric_names()
    label = None

    if normalize_to and normalize_to in table:
        baseline = table[normalize_to]
        resolved: Table = {}
        for col, col_metrics in table.items():
            resolved[col] = {}
            for m in all_metrics:
                if m in col_metrics and m in baseline:
                    resolved[col][m] = col_metrics[m].normalized_to(baseline[m])
        table = resolved
        label = f"Relative to {normalize_to}"

    return table, all_metrics, label


def comparison_bar(
    data: FigureData,
    metrics: Sequence[str] | None = None,
    normalize_to: str | None = None,
    title: str | None = None,
    ylabel: str | None = None,
    legend: bool = True,
    colors: Sequence[str] | None = None,
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
    if colors is None:
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
    if legend:
        ax.legend()
    fig.tight_layout()
    return fig


def comparison_hbar(
    data: FigureData,
    metrics: Sequence[str] | None = None,
    normalize_to: str | None = None,
    title: str | None = None,
    xlabel: str | None = None,
    legend: bool = True,
    colors: Sequence[str] | None = None,
    ax: Axes | None = None,
) -> Figure:
    """Horizontal bar chart with columns (variants) on the Y-axis.

    Each column gets its own bar position.  When a single metric is
    selected this produces one bar per column; with multiple metrics
    the bars are grouped at each column position and colored by metric.
    """
    table, all_metrics, norm_label = _resolve_table(data, metrics, normalize_to)
    if xlabel is None:
        xlabel = norm_label
    columns = data.column_names
    n_cols = len(columns)
    n_metrics = len(all_metrics)

    fig, ax = _ensure_axes(ax, figsize=(8, max(2, n_cols * 0.5 * max(n_metrics, 1))))

    y = np.arange(n_cols)

    if n_metrics <= 1:
        if colors is None:
            colors = palette(n_cols)
        m = all_metrics[0] if all_metrics else ""
        for i, col in enumerate(columns):
            s = table.get(col, {}).get(m, Scalar(0, 0))
            ax.barh(
                y[i], s.mean, 0.7, xerr=s.stddev,
                label=col, color=colors[i], edgecolor="none",
            )
    else:
        group_h = 0.8
        height = group_h / max(n_metrics, 1)
        metric_colors = colors if colors is not None else palette(n_metrics)
        for j, m in enumerate(all_metrics):
            means = [table.get(col, {}).get(m, Scalar(0, 0)).mean for col in columns]
            errs = [table.get(col, {}).get(m, Scalar(0, 0)).stddev for col in columns]
            offset = (j - n_metrics / 2 + 0.5) * height
            ax.barh(
                y + offset, means, height, xerr=errs,
                label=m, color=metric_colors[j], edgecolor="none",
            )

    ax.set_yticks(y)
    ax.set_yticklabels(columns)
    if xlabel:
        ax.set_xlabel(xlabel)
    if title:
        ax.set_title(title)
    if legend:
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
    legend: bool = True,
    colors: Sequence[str] | None = None,
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
    if colors is None:
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

    if legend:
        legend_patches = [
            mpatches.Patch(color=colors[i], alpha=0.7, label=col)
            for i, col in enumerate(columns)
        ]
        ax.legend(handles=legend_patches)
    fig.tight_layout()
    return fig


def compose(
    panels: Sequence[Panel],
    ncols: int = 1,
    figsize: tuple[float, float] | None = None,
    share_x: bool = False,
    share_y: bool = False,
) -> Figure:
    """Arrange panel callables into a subplot grid.

    Each panel is a callable that accepts ax as a keyword argument,
    e.g. functools.partial(comparison_hbar, data, metrics=["score"]).
    """
    n = len(panels)
    nrows = -(-n // ncols)

    if figsize is None:
        figsize = (8 * ncols, max(2, 2.5 * nrows))

    fig, axes = plt.subplots(
        nrows, ncols,
        figsize=figsize,
        sharex=share_x,
        sharey=share_y,
        squeeze=False,
    )
    flat = axes.flatten()

    for i, panel in enumerate(panels):
        panel(ax=flat[i])

    for j in range(n, len(flat)):
        flat[j].set_visible(False)

    if share_x:
        for i in range(n):
            row = i // ncols
            if row < nrows - 1:
                flat[i].set_xlabel("")

    if share_y:
        for i in range(n):
            col = i % ncols
            if col > 0:
                flat[i].set_ylabel("")

    fig.tight_layout()
    return fig


def ranked_cdf(
    data: FigureData,
    metric: str,
    title: str | None = None,
    xlabel: str | None = None,
    ylabel: str | None = None,
    thresholds: Sequence[float] | None = None,
    log_x: bool = False,
    colors: Sequence[str] | None = None,
    ax: Axes | None = None,
) -> Figure:
    """Cumulative distribution of a ranked sequence across columns.

    For each column, reads ``metric`` as a sequence of Scalars (sorted
    by rank, highest first) and plots the cumulative sum normalized to
    [0, 1].  Useful for Pareto / power-law visualizations: the steeper
    the curve, the more concentrated the distribution.

    ``thresholds`` draws horizontal reference lines (e.g. [0.5, 0.9, 0.95]).
    """
    columns = data.column_names
    if colors is None:
        colors = palette(len(columns))
    if thresholds is None:
        thresholds = []

    fig, ax = _ensure_axes(ax)

    for i, col in enumerate(columns):
        m = data.columns[col]
        seq = None
        if m.children and metric in m.children:
            seq = m.children[metric].sequence
        elif m.sequence and not metric:
            seq = m.sequence
        if seq is None:
            continue

        means = np.array([s.mean for s in seq])
        total = means.sum()
        if total == 0:
            continue
        cumulative = np.cumsum(means) / total
        ranks = np.arange(1, len(cumulative) + 1)
        label = f"{col} ({len(means)})"
        ax.plot(ranks, cumulative, label=label, color=colors[i], linewidth=1.5, alpha=0.7)

    for t in thresholds:
        ax.axhline(t, color="#888888", linewidth=0.8, linestyle=":", alpha=0.6)
        ax.text(
            ax.get_xlim()[1] * 0.98, t + 0.01, f"{t:.0%}",
            ha="right", va="bottom", color="#888888",
        )

    if log_x:
        ax.set_xscale("log")
    if xlabel:
        ax.set_xlabel(xlabel)
    if ylabel:
        ax.set_ylabel(ylabel)
    if title:
        ax.set_title(title)
    if len(columns) > 1:
        ax.legend(
            loc="center left", bbox_to_anchor=(1.02, 0.5),
            ncol=1, frameon=False,
        )
    fig.tight_layout()
    return fig


def ranked_cdf_band(
    data: FigureData,
    metric: str,
    title: str | None = None,
    xlabel: str | None = None,
    ylabel: str | None = None,
    thresholds: Sequence[float] | None = None,
    log_x: bool = False,
    outlier_std: float = 2.0,
    ax: Axes | None = None,
) -> Figure:
    columns = data.column_names
    if thresholds is None:
        thresholds = []

    curves: list[tuple[str, np.ndarray, np.ndarray]] = []
    for col in columns:
        m = data.columns[col]
        seq = None
        if m.children and metric in m.children:
            seq = m.children[metric].sequence
        elif m.sequence and not metric:
            seq = m.sequence
        if seq is None:
            continue
        means = np.array([s.mean for s in seq])
        total = means.sum()
        if total == 0:
            continue
        cumulative = np.cumsum(means) / total
        ranks = np.arange(1, len(cumulative) + 1)
        curves.append((col, ranks, cumulative))

    if not curves:
        fig, ax = _ensure_axes(ax)
        return fig

    max_rank = max(len(c) for _, _, c in curves)
    shared_ranks = np.logspace(0, np.log10(max_rank), 200)

    interpolated = np.zeros((len(curves), len(shared_ranks)))
    for i, (_, ranks, cum) in enumerate(curves):
        interpolated[i] = np.interp(shared_ranks, ranks, cum, left=0, right=1)

    median = np.median(interpolated, axis=0)
    lo = np.min(interpolated, axis=0)
    hi = np.max(interpolated, axis=0)
    mean_curve = np.mean(interpolated, axis=0)
    std_curve = np.std(interpolated, axis=0)

    area_under = np.trapezoid(interpolated, shared_ranks, axis=1)
    idx_min = int(np.argmin(area_under))
    idx_max = int(np.argmax(area_under))
    outlier_indices = sorted(set([idx_min, idx_max]))
    outliers = []
    for i in outlier_indices:
        col, ranks, cum = curves[i]
        outliers.append((col, len(cum), ranks, cum))

    _BAND_COLOR = "#2E86AB"
    _OUTLIER_COLORS = ["#7CB342", "#C06078"]

    fig, ax = _ensure_axes(ax)

    ax.fill_between(
        shared_ranks, lo, hi,
        alpha=0.2, color=_BAND_COLOR, label="min\u2013max envelope",
    )
    n_sites = len(curves)
    ax.plot(
        shared_ranks, median,
        color=_BAND_COLOR, linewidth=2,
        label=f"median (n={n_sites})",
    )

    for i, (col, n, ranks, cum) in enumerate(outliers):
        ax.plot(
            ranks, cum,
            color=_OUTLIER_COLORS[i % len(_OUTLIER_COLORS)],
            linewidth=1.5, linestyle="--",
            label=f"{col} ({n})",
        )

    for t in thresholds:
        ax.axhline(t, color="#888888", linewidth=0.8, linestyle=":", alpha=0.6)
        ax.text(
            ax.get_xlim()[1] * 0.98, t + 0.01, f"{t:.0%}",
            ha="right", va="bottom", color="#888888",
        )

    if log_x:
        ax.set_xscale("log")
    if xlabel:
        ax.set_xlabel(xlabel)
    if ylabel:
        ax.set_ylabel(ylabel)
    if title:
        ax.set_title(title)
    ax.legend(loc="lower right")
    fig.tight_layout()
    return fig


def comparison_table(
    row_labels: Sequence[str],
    col_labels: Sequence[str],
    cells: Sequence[Sequence[str]],
    title: str | None = None,
    col_widths: Sequence[float] | None = None,
    row_label_width: float = 0.18,
    fontsize: int = 11,
    header_color: str = "#2E86AB",
    header_text_color: str = "white",
    row_label_color: str = "#f5f5f5",
    stripe_colors: tuple[str, str] = ("white", "#fafafa"),
    ax: Axes | None = None,
) -> Figure:
    """Render a clean, publication-quality table.

    Unlike other plot functions, this takes pre-formatted cell strings
    rather than FigureData, making it composable with any data reshaping
    the caller needs.

    Parameters
    ----------
    row_labels : row header strings (leftmost column)
    col_labels : column header strings (top row)
    cells : 2D list of pre-formatted cell strings, shape (n_rows, n_cols)
    col_widths : relative width per data column (auto if None)
    row_label_width : fraction of figure width for the row label column
    """
    n_rows = len(row_labels)
    n_cols = len(col_labels)

    if col_widths is None:
        data_width = 1.0 - row_label_width
        col_widths = [data_width / max(n_cols, 1)] * n_cols

    fig_w = max(5.5, 2.0 * (n_cols + 1))
    fig_h = max(1.2, 0.40 * (n_rows + 1.5))
    fig, ax = _ensure_axes(ax, figsize=(fig_w, fig_h))
    ax.axis("off")

    cell_colors = []
    for i in range(n_rows):
        bg = stripe_colors[i % 2]
        cell_colors.append([bg] * n_cols)

    tbl = ax.table(
        cellText=cells,
        rowLabels=row_labels,
        colLabels=col_labels,
        cellColours=cell_colors,
        loc="center",
        cellLoc="center",
        colWidths=list(col_widths),
    )
    tbl.auto_set_font_size(False)
    tbl.set_fontsize(fontsize)
    tbl.scale(1.0, 1.6)

    for (r, c), cell in tbl.get_celld().items():
        cell.set_linewidth(0.4)
        cell.set_edgecolor("#d0d0d0")

        if r == 0 and c >= 0:
            cell.set_facecolor(header_color)
            cell.set_text_props(color=header_text_color, weight="bold",
                                fontsize=fontsize)
        elif c == -1:
            cell.set_facecolor(row_label_color)
            cell.set_text_props(ha="right", weight="semibold",
                                fontsize=fontsize - 1)
        else:
            cell.set_text_props(fontsize=fontsize)

    if title:
        ax.set_title(title, pad=16, fontsize=fontsize + 3, weight="bold")

    fig.tight_layout(rect=[0, 0, 1, 0.95] if title else [0, 0, 1, 1])
    return fig


def _ensure_axes(
    ax: Axes | None, figsize: tuple[float, float] | None = None,
) -> tuple[Figure, Axes]:
    if ax is not None:
        fig = ax.get_figure()
        assert fig is not None
        return fig, ax
    return plt.subplots(figsize=figsize)
