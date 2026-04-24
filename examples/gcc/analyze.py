#!/usr/bin/env python3
"""Extract wall time and gcc -ftime-report phase breakdown from a fossil observation.

Receives a single observation as JSON on stdin. Parses stderr for
-ftime-report lines and emits phase wall times alongside the overall
wall_time_ms.
"""
import json, sys, re

obs = json.load(sys.stdin)

metrics = {"wall_time_ms": obs.get("wall_time_us", 0) / 1000.0}

phase_re = re.compile(
    r"^\s+(.+?)\s*:\s+(\d+\.\d+)\s+\("
)

for line in obs.get("stderr", []):
    m = phase_re.match(line)
    if m:
        name = m.group(1).strip()
        if name.startswith("phase "):
            name = name[6:]
        name = name.replace(" ", "_")
        wall_sec = float(m.group(2))
        metrics[f"phase_{name}_ms"] = wall_sec * 1000.0

json.dump(metrics, sys.stdout)
