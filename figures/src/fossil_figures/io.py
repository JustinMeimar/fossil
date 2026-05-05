from __future__ import annotations

import json
import sys

from fossil_figures.types import FigureData, Metric, Scalar


def _parse_metric(raw: object) -> Metric:
    if isinstance(raw, dict):
        if "mean" in raw and "stddev" in raw and len(raw) == 2:
            return Metric(scalar=Scalar(mean=raw["mean"], stddev=raw["stddev"]))
        children = {k: _parse_metric(v) for k, v in raw.items()}
        return Metric(children=children)
    if isinstance(raw, (int, float)):
        return Metric(scalar=Scalar(mean=float(raw), stddev=0.0))
    return Metric()


def load_stdin() -> FigureData:
    raw = json.load(sys.stdin)
    columns = {name: _parse_metric(value) for name, value in raw.items()}
    return FigureData(columns=columns)
