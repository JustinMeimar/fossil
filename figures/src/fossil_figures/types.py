from __future__ import annotations

from dataclasses import dataclass
from typing import TypeAlias

MetricTree: TypeAlias = dict[str, "Metric"]


@dataclass(frozen=True, slots=True)
class Scalar:
    mean: float
    stddev: float

    @property
    def cv(self) -> float:
        """Coefficient of variation."""
        if self.mean == 0:
            return 0.0
        return self.stddev / abs(self.mean)

    def normalized_to(self, baseline: Scalar) -> Scalar:
        """Return this scalar as a ratio relative to baseline."""
        if baseline.mean == 0:
            return Scalar(0.0, 0.0)
        ratio = self.mean / baseline.mean
        # Propagate uncertainty via first-order approximation
        rel_err = ((self.stddev / self.mean) ** 2 + (baseline.stddev / baseline.mean) ** 2) ** 0.5
        return Scalar(ratio, ratio * rel_err)


@dataclass(frozen=True, slots=True)
class Metric:
    """A node in the metric tree. Either a leaf scalar or a nested map."""

    scalar: Scalar | None = None
    children: MetricTree | None = None

    @property
    def is_leaf(self) -> bool:
        return self.scalar is not None

    def walk_scalars(self, prefix: str = "") -> list[tuple[str, Scalar]]:
        """Flatten nested metrics into (dotted.path, scalar) pairs."""
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
    """Parsed figure input: a mapping of column names to metric trees."""

    columns: dict[str, Metric]

    @property
    def column_names(self) -> list[str]:
        return list(self.columns.keys())

    def flat_table(self) -> dict[str, dict[str, Scalar]]:
        """Return {column: {metric_path: scalar}} for tabular access."""
        table: dict[str, dict[str, Scalar]] = {}
        for col, metric in self.columns.items():
            table[col] = dict(metric.walk_scalars())
        return table

    def metric_names(self) -> list[str]:
        """All unique metric paths across columns."""
        names: set[str] = set()
        for metric in self.columns.values():
            for path, _ in metric.walk_scalars():
                names.add(path)
        return sorted(names)
