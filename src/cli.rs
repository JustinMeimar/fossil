use std::path::Path;

pub enum Actions {
    /// Create a .fossil config in pwd
    /// `fossil init`
    Init,

    /// Track a file or pattern of files 
    /// `fossil track *.log`
    Track,
    
    /// Burry all files specified in .fossil
    /// `fossil burry`
    Burry,

    /// Restore the burried artifacts at depth n
    /// `fossil dig 5`
    Dig,

    /// List the artifacts beneath the surface.
    /// `fossil list`
    List
}

/// CLI Args for Fossil. 
pub struct CLIArgs {
    /// The path to the fossil config .fossil 
    pub fossil_config: Path
}

