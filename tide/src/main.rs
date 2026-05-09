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

use crossterm::{cursor::SetCursorStyle, event::{DisableMouseCapture, EnableMouseCapture}};

use crate::application::App;

use std::{default, io::{self, stdout}, panic};

/*****************************************************
 * Main Entry Point
 *****************************************************/

fn main() -> io::Result<()> {
    
    let default_hook = panic::take_hook();

    panic::set_hook(Box::new(move |panic_info| {
        crossterm::execute!(stdout(), DisableMouseCapture);
        default_hook(panic_info);
    }));
    crossterm::execute!(stdout(), EnableMouseCapture);
    crossterm::execute!(stdout(), SetCursorStyle::BlinkingBar);

    let mut terminal = ratatui::init();
    let result = App::new().run(&mut terminal);
    
    

    crossterm::execute!(stdout(), DisableMouseCapture);

    ratatui::restore();
         
    result
}
