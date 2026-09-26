fossil
======

an organization framework for research artifacts. 

fundamentally, fossil is just a wrapper that tracks the metadata around a
command invocation. fossil imposes organization of results into a git
versioned directory recording:

  * what the command actually was which produced this artifact
  * date and time of command
  * git state of the binary being measured (if built from src)
  * cpu configuration
  * result of command (stdout, stderr, wall time, exit status)

this prevents two problems:

  1) scattered accumulation of results
  2) not being able to trust artifacts because their origin
     story has been obfuscated -- "how did i generate this again?"


architecture
------------

all experiments are broken into separate stages. the goal is to let any
experiment be replayable, while decoupling analysis of command outputs
from their invocation -- decreasing expensive re-runs.

1. first is the `record`, the raw capture of a command. recording a record
   can be parameterized by one of n variants, typically corresponding to
   running the subject binary different flags to isolate some measurable
   delta (e.g clang -03 v.s clang -01).

2. once a record has been captured, an `analysis` script, which is
   registered in the experiments toml, parses the raw stdout and stderr of
   the record into a json format. multiple records can be selected for
   analysis and `fossil` will automatically derive the first and second
   moment within a variant. for instance, if the user has collected ten
   invocations of clang -03 and another ten of clang -01 over some program
   and recorded the wall time, fossil will derive the std err.

3. artifact scripts consume analyzed records and emit pdf or json files
   into the project artifact directory. static artifacts omit analysis
   and run without records or stdin. json can be consumed by a typst
   `jsontotable` function.

   having a staged pipeline from experiment all the way to table of results
   lets one back numerical figures in a paper directly with a json table
   that refreshes every time fossil regenerates it. the idea here is to
   never require writing raw numerical figures in paper prose, as doing so
   creates duplicate sources of truth and is ripe for error.

   a headline result in an abstract, for example, may look like: 

   ```
    ambermonkey improves throughput by #sp3-aot-speedup over bytecode-only
    execution and reaches #sp3-aot-default-fraction of default tiered-jit
    throughput. 
   ```

   where in an imported `constants.typ` file one has:

   ```
   #let sp3-aot-default-fraction =
    pct(json-field("7-3-ambermonkey-perf-perf.json", "aot_over_default_ratio"))
   
   #let sp3-aot-speedup =
    pct(json-field("7-3-ambermonkey-perf-perf.json", "aot_over_interp_speedup"))
   ```

   with `7-3-ambermonkey-perf-perf.json` being the direct fossil output for a particular
   experiment. rerun, the paper automatically updates itself. no more
   stale metrics!


runner scripts
--------------

Map each variant to an executable script, relative to the fossil directory.
Variants can share a script or use different scripts:

```toml
[variants]
speedometer3 = "record_browser.py"
jetstream3 = "record_browser.py"
octane = "record_shell.py"
```

Fossil runs the script from the fossil directory with the variant name as
its first argument. The script must have a shebang and executable permission.

artifact configuration
----------------------

set `artifact_dir = "artifacts"` in project.toml, then configure fossil.toml:

```toml
[artifacts.throughput]
script = "plot.py"
analysis = "performance"

[artifacts.summary]
script = "summary.py"
analysis = "performance"

[artifacts.methodology]
script = "methodology.py"
```

nomenclature
------------

fossil draws on some paleontology nomenclature. the goal,
afterall, is to make arbitrarily old research-artifcacts
scrutable. what better analogy than digging up old fossils?

  project   -- a dig site, composed of various fossil types.
               e.g. the spidermonkey dig site.
  fossil    -- an artifact from a particular workload.
               e.g. the octane fossil, the speedometer3 fossil.
  variant   -- a fossil produced under specific conditions.
               e.g. an octane fossil of the --no-ion variant
               (highest optimization tier disabled).

sub-commands are verbs like:

  * `fossil bury` - to run a command and bury a fossil for it. 
  * `fossil dig` - to dig up an old fossil and view it

along with some more reasonable names like:

  * `fossil analyze`
  * `fossil compare`
  * `fossil help` :)


note
----

this is all written in lowercase to convey it was written by a human, is
this effective? who knows.
