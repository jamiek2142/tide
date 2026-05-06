/*****************************************************
 * Copyright 2026, Tide Project
 *****************************************************/

/*****************************************************
 * Crates 
 *****************************************************/

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::env;

/*****************************************************
 * Types
 *****************************************************/

#[derive(Default, Clone)]
pub struct Shell {
    cwd: PathBuf,
    env: HashMap<String, String>,
}

/*****************************************************
 * Implementations
 *****************************************************/

impl Shell {
    pub fn new() -> Self {
        Self {
            cwd: env::current_dir().unwrap_or_default(),
            env: HashMap::new(),
        }
    }

    pub fn set_cwd (&mut self, path : PathBuf)
    {
        self.cwd = path;
    }

    pub fn get_cwd (& self) -> &Path
    {   
        &self.cwd 
    }

    pub fn cwd_as_path (& self) -> PathBuf
    {
        self.clone().cwd
    }

}

