from fossil_figures.types import Scalar, Metric, FigureData
from fossil_figures.io import load_stdin, load_file
from fossil_figures.style import apply_style, FOSSIL_STYLE
from fossil_figures.plot import comparison_bar, timeline, distribution

__all__ = [
    "Scalar",
    "Metric",
    "FigureData",
    "load_stdin",
    "load_file",
    "apply_style",
    "FOSSIL_STYLE",
    "comparison_bar",
    "timeline",
    "distribution",
]
