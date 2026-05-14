from __future__ import annotations

import matplotlib as mpl
import matplotlib.pyplot as plt

_FONT_KEYS = [
    "font.size",
    "axes.titlesize",
    "axes.labelsize",
    "xtick.labelsize",
    "ytick.labelsize",
    "legend.fontsize",
]

FOSSIL_STYLE: dict[str, object] = {
    "font.family": "serif",
    "font.serif": ["Computer Modern Roman", "DejaVu Serif"],
    "font.size": 14,
    "axes.titlesize": 16,
    "axes.labelsize": 14,
    "xtick.labelsize": 12,
    "ytick.labelsize": 12,
    "legend.fontsize": 12,
    "figure.figsize": (6.4, 4.0),
    "figure.dpi": 150,
    "savefig.dpi": 300,
    "savefig.bbox": "tight",
    "axes.spines.top": False,
    "axes.spines.right": False,
    "axes.grid": True,
    "grid.alpha": 0.3,
    "grid.linestyle": "--",
    "text.usetex": False,
    "errorbar.capsize": 3,
}

COLUMN_PRESETS: dict[str, dict[str, object]] = {
    "single": {
        "figure.figsize": (3.33, 2.4),
        "font.size": 8,
        "axes.titlesize": 9,
        "axes.labelsize": 8,
        "xtick.labelsize": 7,
        "ytick.labelsize": 7,
        "legend.fontsize": 7,
    },
    "double": {
        "figure.figsize": (7.0, 3.5),
        "font.size": 10,
        "axes.titlesize": 11,
        "axes.labelsize": 10,
        "xtick.labelsize": 9,
        "ytick.labelsize": 9,
        "legend.fontsize": 9,
    },
}


def apply_style(
    font_scale: float = 1.0,
    column: str | None = None,
) -> None:
    """Apply fossil figure style.

    Parameters
    ----------
    font_scale : multiplicative scale applied to all font sizes.
    column : "single" or "double" for publication column-width presets.
             Overrides figsize and font sizes to match typical two-column
             paper layouts (3.33in single, 7.0in double).
    """
    style = dict(FOSSIL_STYLE)

    if column and column in COLUMN_PRESETS:
        style.update(COLUMN_PRESETS[column])

    if font_scale != 1.0:
        for key in _FONT_KEYS:
            style[key] = round(style[key] * font_scale)  # type: ignore[arg-type]

    mpl.rcParams.update(style)  # type: ignore[arg-type]


def palette(n: int) -> list[str]:
    """Return n visually distinct colors from the fossil palette."""
    base = [
        "#2E86AB",  # steel blue
        "#A23B72",  # plum
        "#F18F01",  # amber
        "#C73E1D",  # rust
        "#3B1F2B",  # dark plum
        "#44AF69",  # green
        "#6B4C9A",  # purple
    ]
    if n <= len(base):
        return base[:n]
    cmap = plt.get_cmap("tab20")
    return [mpl.colors.to_hex(cmap(i / n)) for i in range(n)]
