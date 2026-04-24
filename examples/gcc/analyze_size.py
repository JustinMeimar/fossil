#!/usr/bin/env python3
"""Parse `size` output from stdout into section size metrics.

`size` outputs:
   text	   data	    bss	    dec	    hex	filename
   2028	    656	134217760	134220444	8000a9c	/tmp/bench

Emits: text_bytes, data_bytes, bss_bytes, total_bytes
"""
import json, sys

obs = json.load(sys.stdin)

metrics = {}
lines = obs.get("stdout", [])

for line in lines:
    parts = line.split()
    if len(parts) >= 4:
        try:
            text = int(parts[0])
            data = int(parts[1])
            bss = int(parts[2])
            total = int(parts[3])
            metrics["text_bytes"] = text
            metrics["data_bytes"] = data
            metrics["bss_bytes"] = bss
            metrics["total_bytes"] = total
            break
        except ValueError:
            continue

json.dump(metrics, sys.stdout)
