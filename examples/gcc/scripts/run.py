#!/usr/bin/env python3
"""One runner shared by all fossils and variants in this project."""
import os
from pathlib import Path
import subprocess
import shutil
import sys
import tempfile
import tomllib

project = Path(__file__).resolve().parent.parent
with (project / "settings.toml").open("rb") as file:
    settings = tomllib.load(file)

# Fossil runs scripts from the fossil directory and passes the variant in argv[1].
experiment = Path.cwd().name
variant = sys.argv[1]
perf = [settings["perf"], "stat", "-e", ",".join(settings["perf_events"]), "-x,"]
# GNU time uses localized labels; our parser expects English.
environment = dict(os.environ, LC_ALL="C")

with tempfile.TemporaryDirectory(prefix="fossil-gcc-") as temporary:
    binary = Path(temporary) / "workload"
    command = [settings["compiler"], f"-{variant}", *settings["common_flags"],
               str(project / "workload.c"), "-lm", "-o", str(binary)]
    if experiment == "compile":
        command.append("-ftime-report")
    elif experiment == "memory":
        command = [settings["time"], "-v", *command]
    elif experiment == "perf-compile":
        command = [*perf, *command]
    elif experiment not in ("execute", "binary-analysis"):
        raise SystemExit(f"Unknown experiment: {experiment}")
    if shutil.which(command[0]) is None:
        raise SystemExit(f"Required tool not found: {command[0]}; configure it in settings.toml")
    subprocess.run(command, check=True, env=environment)
    if experiment == "execute":
        subprocess.run([*perf, str(binary)], check=True, env=environment)
    elif experiment == "binary-analysis":
        subprocess.run([settings["size"], str(binary)], check=True, env=environment)
