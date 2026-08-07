from __future__ import annotations

import hashlib

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


def font_sizes() -> dict[str, float]:
    """Return resolved font sizes from current rcParams.

    Keys: title, label, tick, cell, cell_bold — the last two are derived
    sizes useful for heatmap / matrix annotations.
    """
    title = float(mpl.rcParams["axes.titlesize"])
    label = float(mpl.rcParams["axes.labelsize"])
    tick = float(mpl.rcParams["xtick.labelsize"])
    return {
        "title": title,
        "label": label,
        "tick": tick,
        "cell": tick,
        "cell_bold": label,
    }


PALETTE = [
    "#2E86AB",  # steel blue
    "#E8553A",  # vermilion
    "#44AF69",  # green
    "#F18F01",  # amber
    "#6B4C9A",  # purple
    "#20B2AA",  # teal
    "#E91E63",  # rose
    "#78909C",  # blue grey
    "#5C6BC0",  # indigo
    "#8D6E63",  # brown
    "#7CB342",  # lime
    "#AB47BC",  # violet
    "#00ACC1",  # cyan
    "#C73E1D",  # rust
    "#FDD835",  # yellow
    "#A23B72",  # plum
]


def get_color(name: str) -> str:
    h = int(hashlib.sha256(name.encode()).hexdigest(), 16)
    return PALETTE[h % len(PALETTE)]


def get_colors(names: list[str]) -> list[str]:
    seen: dict[int, list[int]] = {}
    indices = []
    for i, name in enumerate(names):
        h = int(hashlib.sha256(name.encode()).hexdigest(), 16)
        idx = h % len(PALETTE)
        indices.append(idx)
        seen.setdefault(idx, []).append(i)

    used = set(indices)
    for slots in seen.values():
        if len(slots) <= 1:
            continue
        for dup in slots[1:]:
            for candidate in range(len(PALETTE)):
                if candidate not in used:
                    indices[dup] = candidate
                    used.add(candidate)
                    break

    return [PALETTE[i] for i in indices]


def palette(n: int) -> list[str]:
    if n <= len(PALETTE):
        return PALETTE[:n]
    cmap = plt.get_cmap("tab20")
    return [mpl.colors.to_hex(cmap(i / n)) for i in range(n)]
