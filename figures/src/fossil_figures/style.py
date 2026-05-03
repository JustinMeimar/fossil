from __future__ import annotations

import matplotlib as mpl
import matplotlib.pyplot as plt

FOSSIL_STYLE: dict[str, object] = {
    "font.family": "serif",
    "font.serif": ["Computer Modern Roman", "DejaVu Serif"],
    "font.size": 10,
    "axes.titlesize": 11,
    "axes.labelsize": 10,
    "xtick.labelsize": 9,
    "ytick.labelsize": 9,
    "legend.fontsize": 9,
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


def apply_style() -> None:
    """Apply the fossil figure style globally."""
    mpl.rcParams.update(FOSSIL_STYLE)  # type: ignore[arg-type]


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
