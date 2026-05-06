/*****************************************************
 * Copyright 2026, Tide Project
 *****************************************************/

#![allow(warnings)]

/*****************************************************
 * Modules
 *****************************************************/

mod input;
mod shell;
mod file_system;
mod application;


/*****************************************************
 * Crates
 *****************************************************/

use crate::application::App;

use std::io;

/*****************************************************
 * Main Entry Point
 *****************************************************/

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    
    // TODO: Get result and return it 
    App::new().run(&mut terminal)?;

    ratatui::restore();
    Ok(())
}
