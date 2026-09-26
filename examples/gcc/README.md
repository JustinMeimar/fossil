# GCC example project

Five fossils share runner, analyzer, and report scripts:

| Fossil | Measurement | Extra requirements |
| --- | --- | --- |
| `compile` | Compilation time and GCC phase timings | GCC |
| `binary-analysis` | Executable section sizes | GCC, GNU `size` |
| `memory` | Compiler memory and resource usage | GCC, GNU `time` |
| `perf-compile` | Hardware counters during compilation | GCC, Linux `perf` |
| `execute` | Hardware counters while executing the workload | GCC, Linux `perf` |

All scripts require Python 3.11 or newer. Hardware counters require permission
from the host's perf configuration. `execute` compiles before measuring the
executable with perf: its overall `wall_time_ms` includes compilation, while
its hardware counters describe execution alone.

## Layout

```text
gcc/
├── project.toml            # Fossil's project settings
├── settings.toml           # Settings owned by the example scripts
├── workload.c
├── scripts/
│   ├── run.py              # Shared across fossils and optimization variants
│   ├── analyze.py
│   ├── analyze_size.py
│   ├── analyze_memory.py
│   ├── analyze_perf.py
│   └── report.py
├── fossils/
│   ├── compile/fossil.toml
│   ├── binary-analysis/fossil.toml
│   ├── memory/fossil.toml
│   ├── perf-compile/fossil.toml
│   └── execute/fossil.toml
└── artifacts/              # Created by fossil emit; ignored by git
```

Each fossil stores observations under its own `records/` directory.
Edit `settings.toml` to choose the compiler, tool paths, common flags, or perf events.
Fossil does not interpret this file; `scripts/run.py` reads it directly.

## Run it

From the repository root, register the complete example project with a symlink:

```sh
cargo build
mkdir -p ~/.fossil/projects
ln -s "$(pwd)/examples/gcc" ~/.fossil/projects/gcc

# Choose one variant for a quick first run.
target/debug/fossil --project gcc bury compile --variant O2 -n 2
target/debug/fossil --project gcc analyze compile --analysis metrics
target/debug/fossil --project gcc emit compile --artifact comparison

# Compare all configured optimization levels.
target/debug/fossil --project gcc bury binary-analysis
target/debug/fossil --project gcc emit binary-analysis --artifact comparison

target/debug/fossil --project gcc serve
```

Use an unused project name/location if `~/.fossil/projects/gcc` already exists.
The commands can be run from any working directory. To keep the example tree
clean, copy the whole `examples/gcc` directory elsewhere and register that copy.
`fossil import` handles standalone fossil configs with local scripts; it does not
copy a project's shared settings and source files.

Each `comparison` emission writes three files under
`artifacts/<fossil>/comparison/`: `metrics.json`, `metrics.csv`, and
`comparison.svg`. The web view lists them and previews the text and plot.
The report uses Python's standard library; a report script can instead use a
plotting library to produce PDFs without changing the Fossil configuration.

## Configuration and script contract

`project.toml` sets `name`, optional `description`, and optional `artifact_dir`.
The artifact directory is relative to the project, or may be absolute.
It is required when emitting artifacts.

Each `fossils/<name>/fossil.toml` declares:

```toml
name = "compile"
description = "Compilation time across optimization levels"
default_iterations = 5
allow_failure = false

[variants]
O0 = "../../scripts/run.py"
O2 = "../../scripts/run.py"
O3 = "../../scripts/run.py"

[analyses]
metrics = "../../scripts/analyze.py"

[artifacts.comparison]
script = "../../scripts/report.py"
analysis = "metrics"
```

The fossil name must match its directory. Script paths are relative to
the fossil directory, or may be absolute. Scripts must be executable.
`default_iterations` defaults to 10 and `allow_failure` defaults to false.
Optional `workdir` changes the variant runner's working directory; relative
values resolve from the fossil directory. Other scripts always run from the
fossil directory.

| Stage | Arguments | Standard input | Result |
| --- | --- | --- | --- |
| Variant | Variant key | Inherited | Fossil records wall time, exit status, stdout and stderr |
| Analysis | Variant key, absolute record-directory path | One observation as JSON | JSON metrics on stdout |
| Artifact | Absolute output-directory path | Aggregated analysis JSON, or EOF when `analysis` is omitted | Files written inside that directory |

An observation contains `iteration`, `wall_time_us`, `exit_code`, and arrays of
stdout/stderr lines. Analysis scripts can use the record-directory argument to
read `manifest.json` if they need additional context. Fossil merges numeric
metrics into `{ "mean": ..., "stddev": ... }` and groups the result by variant
(or record identifier when selecting multiple records for a single variant).

Artifact scripts may produce any file formats, including multiple files and
nested directories. Fossil creates the output directory before invoking the
script and requires a successful exit with at least one regular file present.
Re-emission reuses the directory; scripts own replacement and removal of their
files. A failed script may leave partial output. The example report overwrites
its three files. Generated symlinks are not listed or served by the web view.

Scripts inherit the ordinary process environment. Fossil injects no custom
execution variables and performs no variable interpolation in TOML.
`FOSSIL_HOME` remains an option for locating Fossil's own registry.

## Migrating older configs

- Move fossil directories beneath the project's `fossils/` directory.
- Make `name` agree with the fossil directory name.
- Replace command arrays with executable script paths in `[variants]`.
- Use the named `[analyses]` map instead of `analyze`.
- Remove artifact `format`; change artifact scripts to accept an output
  directory instead of a filename.
- Move `[constants]` and their expansion into script-owned settings.
- Replace script reads of injected `FOSSIL_*` variables with arguments,
  script-relative paths, or settings. `emit --force` has been removed;
  scripts decide how to overwrite their output.

The CLI prints the emitted directory instead of launching an editor or viewer.
Open it in the web view or with your preferred application.
