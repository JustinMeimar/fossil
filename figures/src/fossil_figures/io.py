from __future__ import annotations

import json
import sys
from pathlib import Path

from fossil_figures.types import FigureData, Metric, Scalar


def _parse_metric(raw: object) -> Metric:
    """Recursively parse a JSON value into a typed Metric."""
    if isinstance(raw, dict):
        if "mean" in raw and "stddev" in raw and len(raw) == 2:
            return Metric(scalar=Scalar(mean=raw["mean"], stddev=raw["stddev"]))
        children = {k: _parse_metric(v) for k, v in raw.items()}
        return Metric(children=children)
    if isinstance(raw, str):
        return Metric(scalar=Scalar(mean=0.0, stddev=0.0))
    if isinstance(raw, (int, float)):
        return Metric(scalar=Scalar(mean=float(raw), stddev=0.0))
    return Metric(scalar=Scalar(mean=0.0, stddev=0.0))


def _parse(raw: dict[str, object]) -> FigureData:
    columns = {name: _parse_metric(value) for name, value in raw.items()}
    return FigureData(columns=columns)


def load_stdin() -> FigureData:
    """Load figure data from stdin (the fossil protocol)."""
    raw = json.load(sys.stdin)
    return _parse(raw)


def load_file(path: Path | str) -> FigureData:
    """Load figure data from a JSON file (useful for development)."""
    with open(path) as f:
        raw = json.load(f)
    return _parse(raw)
