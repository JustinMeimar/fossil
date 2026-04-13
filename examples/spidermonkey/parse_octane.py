#!/usr/bin/env python3
"""Example analysis script for Octane benchmark results.

Receives a single observation as JSON on stdin, extracts the score
from stdout lines matching "Score: <number>", and prints a JSON
object with the extracted metric.

Usage:
    fossil analyze octane-benchmark --site spidermonkey
"""
import json, sys, re

obs = json.load(sys.stdin)
score = None
for line in obs.get("stdout", []):
    m = re.search(r"Score:\s*(\d+)", line)
    if m:
        score = int(m.group(1))
        break

json.dump({"score": score or 0}, sys.stdout)
