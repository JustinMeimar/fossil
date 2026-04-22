fossil
======

an organization framework for research artifacts.

fundamentally, fossil is just a wrapper that tracks metadata
around a command invocation. the wrapper imposes organization
of results into a git versioned directory recording: 

  * what the command actually was which produced this artifact
  * date and time of command
  * git state of the binary being measured (if built from src)
  * cpu configuration
  * result of command (stdout, stderr, wall time, exit status)

this prevents two problems:

  1) scattered accumulation of results
  2) not being able to trust artifacts because their origin
     story has been obfuscated -- "how did i generate this again?"

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

features
--------

  * projects and fossil configs managed in ~/.fossil
  * automatic git repo init and management across buries
  * associate analysis scripts with fossils to parse and
    create statistics within or across results

roadmap
-------

  * better analysis support
  * better fossil comparisson support
  * under-the-hood integration of [figure-factory](https://github.com/JustinMeimar/figure-factory)
  * serving basic html site to explore fossils (?)
  * tui (?)

