/*****************************************************
 * Copyright 2026, Tide Project
 *****************************************************/

/**
 * This file implements the shell logic
 */ 

/*****************************************************
 * Crates 
 *****************************************************/

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::env;
use std::ffi::OsString;

use crossbeam_channel::Sender;

use portable_pty::{CommandBuilder, NativePtySystem, PtyPair, PtySize, PtySystem};

/*****************************************************
 * Types
 *****************************************************/

#[derive(Clone)]
pub struct Shell {
    cwd : PathBuf,
    env : HashMap<String, String>,
    tx  : Sender<Vec<u8>>,
}

/*****************************************************
 * Implementations
 *****************************************************/

impl Shell {
    pub fn new(tx : Sender<Vec<u8>>) -> Self {
        Self {
            cwd: env::current_dir().unwrap_or_default(),
            env: HashMap::new(),
            tx : tx
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

    pub fn send_cmd(&mut self, argv: Vec<&str>) 
    {
        let pty_system = NativePtySystem::default();
        let pair = pty_system.openpty(PtySize::default()).unwrap();

        let argv = argv.into_iter().map(OsString::from).collect();

        let mut cmd = CommandBuilder::from_argv(argv);
        cmd.cwd(self.get_cwd());
        // TODO: Environment variables.

        let Ok(mut _child) = pair.slave.spawn_command(cmd) else {
            // TODO: Optional clear input.
            return;
        };
        let mut reader = pair.master.try_clone_reader().unwrap();

        let tx = self.tx.clone();

        std::thread::spawn(move || {
            let mut buffer = [0u8; 1024];

            while let Ok(n) = reader.read(&mut buffer) {
                if n == 0 {
                    break;
                }
                let _ = tx.send(buffer[..n].to_vec());
            }
        });

      }

}

