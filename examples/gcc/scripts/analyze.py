#!/usr/bin/env python3
"""Extract a stable set of GCC phase wall times from one observation."""
import json
import re
import sys

observation = json.load(sys.stdin)
# GCC omits phases that take no measurable time. Keep the metric shape stable.
phases = {
    "phase setup": "setup_ms",
    "phase parsing": "parsing_ms",
    "phase opt and generate": "optimization_and_generation_ms",
}
metrics = {"wall_time_ms": observation["wall_time_us"] / 1000.0}
metrics.update({name: 0.0 for name in phases.values()})
row = re.compile(r"^\s+(.+?)\s*:\s+(.*)$")
timing = re.compile(r"(\d+\.\d+)\s+\([^)]*\)")
for line in observation["stderr"]:
    match = row.match(line)
    if match and match[1] in phases:
        # GCC reports either wall alone or user/sys/wall; wall is last.
        times = timing.findall(match[2])
        if times:
            metrics[phases[match[1]]] = float(times[-1]) * 1000.0
json.dump(metrics, sys.stdout)
