from __future__ import annotations

import json
import sys

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
