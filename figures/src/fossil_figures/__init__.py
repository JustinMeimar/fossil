from fossil_figures.types import Scalar, Metric, FigureData
from fossil_figures.io import load_stdin
from fossil_figures.style import apply_style, palette, COLUMN_PRESETS
from fossil_figures.plot import comparison_bar, comparison_hbar, comparison_table, violin, compose, ranked_cdf

__all__ = [
    "Scalar",
    "Metric",
    "FigureData",
    "load_stdin",
    "apply_style",
    "palette",
    "COLUMN_PRESETS",
    "comparison_bar",
    "comparison_hbar",
    "comparison_table",
    "violin",
    "compose",
    "ranked_cdf",
]
