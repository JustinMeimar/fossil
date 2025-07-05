
## Fossil

Fossil is a CLI tool for keeping track of artifacts which can be mistakenly overwritten,
such as binaries and debug logs, etc. Fossil lets you track, burry and dig up such files
at your convenience.

#### Usage

```
  fossil init                   # Layer 0
  
  fossil track file.txt         # Add to tracking
  
  echo "v1" > file.txt 
  fossil burry                  # Layer 1 created
  
  echo "v2" > file.txt
  fossil burry                  # Layer 2 created
  
  fossil dig 1                  # Back to layer 1 (file.txt -> symlink to v1) 
  fossil dig 2                  # Back to layer 0 (file.txt -> symlink to original)
    
  fossil surface                # Restore all tracked fossils back to surface level

  fossil list                   # Shows layers and current state
```

#### Building
```
git clone https://github.com/yourusername/fossil
cd fossil
cargo install --path .
```

