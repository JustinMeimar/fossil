
## Fossil

Fossil is a CLI tool for keeping track of artifacts in a directory which
get prominently overwritten like build binaries and debug logs. Fossil
lets you track, burry and dig up such files at your convenience.

```
  fossil init                   # Layer 0
  fossil track file.txt         # Add to tracking
  echo "v1" > file.txt
  fossil burry                  # Layer 1 created
  echo "v2" > file.txt
  fossil burry                  # Layer 2 created
  fossil dig 1                  # Back to layer 1 (file.txt -> symlink to v1)
  fossil dig 2                  # Back to layer 0 (file.txt -> symlink to
  original)
  fossil list                   # Shows layers and current state
```
