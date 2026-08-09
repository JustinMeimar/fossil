from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any, Iterable, Sequence

from fossil_figures.types import FigureData, Metric, Scalar


def _parse_scalar(raw: dict) -> Scalar:
    return Scalar(mean=raw["mean"], stddev=raw["stddev"])


def _parse_metric(raw: object) -> Metric:
    if isinstance(raw, dict):
        if "mean" in raw and "stddev" in raw and len(raw) == 2:
            return Metric(scalar=_parse_scalar(raw))
        children = {k: _parse_metric(v) for k, v in raw.items()}
        return Metric(children=children)
    if isinstance(raw, list):
        items = []
        for item in raw:
            if isinstance(item, dict) and "mean" in item and "stddev" in item:
                items.append(_parse_scalar(item))
            elif isinstance(item, (int, float)):
                items.append(Scalar(mean=float(item), stddev=0.0))
            else:
                return Metric()
        return Metric(sequence=items)
    if isinstance(raw, (int, float)):
        return Metric(scalar=Scalar(mean=float(raw), stddev=0.0))
    if isinstance(raw, str):
        return Metric(tag=raw)
    return Metric()


def load_stdin() -> FigureData:
    raw = json.load(sys.stdin)
    columns = {name: _parse_metric(value) for name, value in raw.items()}
    return FigureData(columns=columns)


_ALLOWED_FORMATS = {"str", "int", "float", "percent"}
_ALLOWED_ALIGNS = {"left", "right", "center"}


def write_typst_table(
    path: str | Path,
    columns: Sequence[dict[str, Any]],
    rows: Iterable[Sequence[Any]],
    *,
    title: str | None = None,
    text_size: float | None = None,
    cell_inset: float | None = None,
    column_weights: Sequence[float] | None = None,
) -> None:
    """Emit the JSON schema consumed by render-table() in the typst lib.

    columns: list of {key, label, format?, align?} dicts, one per column.
    rows:    iterable of row-lists (values only; format is per-column).
    """
    cols: list[dict[str, Any]] = []
    for i, col in enumerate(columns):
        if "key" not in col or "label" not in col:
            raise ValueError(f"column {i} missing 'key' or 'label'")
        fmt = col.get("format", "str")
        if fmt not in _ALLOWED_FORMATS:
            raise ValueError(
                f"column {col['key']}: format {fmt!r} not in {sorted(_ALLOWED_FORMATS)}"
            )
        align = col.get("align", "left")
        if align not in _ALLOWED_ALIGNS:
            raise ValueError(
                f"column {col['key']}: align {align!r} not in {sorted(_ALLOWED_ALIGNS)}"
            )
        cols.append(
            {"key": col["key"], "label": col["label"], "format": fmt, "align": align}
        )

    materialised = [list(r) for r in rows]
    for r_idx, row in enumerate(materialised):
        if len(row) != len(cols):
            raise ValueError(
                f"row {r_idx} has {len(row)} cells, expected {len(cols)}"
            )

    if column_weights is not None and len(column_weights) != len(cols):
        raise ValueError(
            f"column_weights has {len(column_weights)} entries, expected {len(cols)}"
        )

    payload: dict[str, Any] = {"columns": cols, "rows": materialised}
    if title is not None:
        payload["title"] = title
    if text_size is not None:
        payload["text_size"] = text_size
    if cell_inset is not None:
        payload["cell_inset"] = cell_inset
    if column_weights is not None:
        payload["column_weights"] = list(column_weights)

    Path(path).write_text(json.dumps(payload, indent=2, sort_keys=False) + "\n")
