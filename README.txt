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

3. analyzed records are consumed by either a `figure` script or a `table`
   script. the former will produce a pdf figure, suitable for a paper. the
   later will emit a json table in a fixed column, row format, which can
   also by consumed by a typst `jsontotable` function.

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
    pct(json-field("7-3-perf.json", "aot_over_default_ratio"))
   
   #let sp3-aot-speedup =
    pct(json-field("7-3-perf.json", "aot_over_interp_speedup"))
   ```

   with `7-3-perf.json` being the direct fossil output for a particular
   experiment. rerun, the paper automatically updates itself. no more
   stale metrics!


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
