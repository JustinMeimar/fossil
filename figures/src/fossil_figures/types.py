from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class Scalar:
    mean: float
    stddev: float

    def normalized_to(self, baseline: Scalar) -> Scalar:
        if baseline.mean == 0:
            return Scalar(0.0, 0.0)
        ratio = self.mean / baseline.mean
        rel_err = ((self.stddev / self.mean) ** 2 + (baseline.stddev / baseline.mean) ** 2) ** 0.5
        return Scalar(ratio, ratio * rel_err)


@dataclass(frozen=True, slots=True)
class Metric:
    scalar: Scalar | None = None
    children: dict[str, Metric] | None = None

    def walk_scalars(self, prefix: str = "") -> list[tuple[str, Scalar]]:
        results: list[tuple[str, Scalar]] = []
        if self.scalar is not None:
            results.append((prefix, self.scalar))
        if self.children is not None:
            for key, child in self.children.items():
                path = f"{prefix}.{key}" if prefix else key
                results.extend(child.walk_scalars(path))
        return results


@dataclass(frozen=True, slots=True)
class FigureData:
    columns: dict[str, Metric]

    @property
    def column_names(self) -> list[str]:
        return list(self.columns.keys())

    def flat_table(self) -> dict[str, dict[str, Scalar]]:
        table: dict[str, dict[str, Scalar]] = {}
        for col, metric in self.columns.items():
            table[col] = dict(metric.walk_scalars())
        return table

    def metric_names(self) -> list[str]:
        names: set[str] = set()
        for metric in self.columns.values():
            for path, _ in metric.walk_scalars():
                names.add(path)
        return sorted(names)
