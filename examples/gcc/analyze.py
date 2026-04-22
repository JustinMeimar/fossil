#!/usr/bin/env python3
"""Extract wall time in milliseconds from a fossil observation.

Receives a single observation as JSON on stdin, converts wall_time_us
to milliseconds, and prints a JSON object with the metric.

Usage:
    fossil analyze compile --project gcc
"""
import json, sys

obs = json.load(sys.stdin)
wall_ms = obs.get("wall_time_us", 0) / 1000.0
json.dump({"wall_time_ms": wall_ms}, sys.stdout)
