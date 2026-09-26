#!/usr/bin/env python3
"""Parse /usr/bin/time -v stderr output into resource metrics.

/usr/bin/time -v emits lines like:
    Maximum resident set size (kbytes): 12345
    Minor (reclaiming a frame) page faults: 6789
    Voluntary context switches: 42

Emits: wall_time_ms, peak_rss_kb, minor_faults, voluntary_ctx_switches
"""
import json, sys, re

obs = json.load(sys.stdin)

metrics = {"wall_time_ms": obs.get("wall_time_us", 0) / 1000.0}

patterns = {
    "peak_rss_kb": r"Maximum resident set size \(kbytes\):\s+(\d+)",
    "minor_faults": r"Minor \(reclaiming a frame\) page faults:\s+(\d+)",
    "major_faults": r"Major \(requiring I/O\) page faults:\s+(\d+)",
    "voluntary_ctx_switches": r"Voluntary context switches:\s+(\d+)",
    "involuntary_ctx_switches": r"Involuntary context switches:\s+(\d+)",
}

for line in obs.get("stderr", []):
    for metric_name, pattern in patterns.items():
        m = re.search(pattern, line)
        if m:
            metrics[metric_name] = int(m.group(1))

json.dump(metrics, sys.stdout)
