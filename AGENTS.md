
<philosophy>

Fossil follows the "functional core, imperative shell" pattern.

All side effects — filesystem I/O, subprocess spawning, env var reads,
sysfs reads, printing — belong at the edges of the application. The
interior should be pure: stateless functions that take data in and
return data out, with no hidden syscalls or mutation.

Concretely:
- Constructors must not do I/O. If a struct needs system state, the
  caller gathers it and passes it in.
- Prefer well-named verb types (Run, Environment) that carry pure
  state between pipeline stages. The types are the finite state
  machine; side effects happen at transitions between states, not
  inside them.
- Format functions return data (Vec<String>, BTreeMap), not print.
  The shell decides how to present.
- Keep impure orchestration thin: load, gather, call pure logic,
  write results. Each step is visible at the call site.

</philosophy>

<style>

</style>

