#!/usr/bin/env python3
"""Write the full analysis, a numeric table, and an SVG plot using only Python."""
import csv
from html import escape
import json
from pathlib import Path
import sys

result = json.load(sys.stdin)
output = Path(sys.argv[1])
# The directory already exists. Re-emission replaces the files we own.
(output / "metrics.json").write_text(json.dumps(result, indent=2) + "\n")
with (output / "metrics.csv").open("w", newline="") as file:
    writer = csv.writer(file)
    writer.writerow(["variant", "metric", "mean", "stddev"])
    for variant, metrics in result.items():
        for metric, value in metrics.items():
            writer.writerow([variant, metric, value["mean"], value["stddev"]])

# Prefer size for the binary-size fossil and overall elapsed time otherwise.
first = next(iter(result.values()))
metric = "total_bytes" if "total_bytes" in first else "wall_time_ms"
rows = [(variant, metrics[metric]["mean"]) for variant, metrics in result.items()]
maximum = max((value for _, value in rows), default=0) or 1
height = 70 + 40 * len(rows)
svg = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 {height}">',
       '<rect width="100%" height="100%" fill="white"/>',
       f'<text x="20" y="30" font-family="sans-serif">{escape(metric)} (mean)</text>']
for index, (variant, value) in enumerate(rows):
    y = 55 + index * 40
    width = 500 * value / maximum
    svg.extend([
        f'<text x="20" y="{y + 18}" font-family="sans-serif">{escape(variant)}</text>',
        f'<rect x="120" y="{y}" width="{width}" height="25" fill="#487cba"/>',
        f'<text x="{130 + width}" y="{y + 18}" font-family="sans-serif">{value:.3f}</text>',
    ])
svg.append('</svg>')
(output / "comparison.svg").write_text("\n".join(svg) + "\n")
