use std::path::PathBuf;

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

    /// Restore the burried artifacts at layer n
    /// `fossil dig 5`
    Dig,
    
    /// Restore the level of all burried files to the surface.
    /// `fossil surface`
    Surface,

    /// List the artifacts beneath the surface.
    /// `fossil list`
    List
}

/// CLI Args for Fossil. 
pub struct CLIArgs {
    /// The path to the fossil config .fossil 
    pub fossil_config: PathBuf,
    pub action: Actions
}

