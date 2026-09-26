#!/usr/bin/env python3
"""Parse perf stat -x, CSV output from stderr into metrics.

perf stat -x, writes comma-separated lines to stderr:
    counter-value,unit,event-name,run-time,pct,...

Emits raw counters plus derived ratios (IPC, cache/branch miss rates).
"""
import json, sys

obs = json.load(sys.stdin)

counters = {}
for line in obs.get("stderr", []):
    parts = line.split(",")
    if len(parts) < 3:
        continue
    try:
        value = int(parts[0])
    except (ValueError, IndexError):
        continue
    event = parts[2].strip()
    counters[event] = value

metrics = {"wall_time_ms": obs.get("wall_time_us", 0) / 1000.0}

for event in ("cycles", "instructions", "cache-references",
              "cache-misses", "branches", "branch-misses"):
    if event in counters:
        metrics[event.replace("-", "_")] = counters[event]

if "cycles" in counters and counters["cycles"] > 0:
    metrics["ipc"] = counters.get("instructions", 0) / counters["cycles"]

if "cache-references" in counters and counters["cache-references"] > 0:
    metrics["cache_miss_rate"] = (
        counters.get("cache-misses", 0) / counters["cache-references"]
    )

if "branches" in counters and counters["branches"] > 0:
    metrics["branch_miss_rate"] = (
        counters.get("branch-misses", 0) / counters["branches"]
    )

json.dump(metrics, sys.stdout)
