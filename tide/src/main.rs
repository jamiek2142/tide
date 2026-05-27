/*****************************************************
 * Copyright 2026, Tide Project
 *****************************************************/

/*****************************************************
 * Modules
 *****************************************************/

mod application;
mod file_system;
mod input;
mod popup_menu;
mod search;
mod shell;

/*****************************************************
 * Crates
 *****************************************************/

use crossterm::{
    cursor::SetCursorStyle,
    event::{DisableMouseCapture, EnableMouseCapture},
};

use crate::application::App;

use std::{
    io::{self, stdout},
    panic,
};

/*****************************************************
 * Main Entry Point
 *****************************************************/

fn main() -> io::Result<()> {
    let default_hook = panic::take_hook();

    panic::set_hook(Box::new(move |panic_info| {
        let _ = crossterm::execute!(stdout(), DisableMouseCapture);
        default_hook(panic_info);
    }));

    let _ = crossterm::execute!(stdout(), EnableMouseCapture);
    let _ = crossterm::execute!(stdout(), SetCursorStyle::BlinkingBar);

    let mut terminal = ratatui::init();
    let result = App::new().run(&mut terminal);

    let _ = crossterm::execute!(stdout(), SetCursorStyle::DefaultUserShape);
    let _ = crossterm::execute!(stdout(), DisableMouseCapture);

    ratatui::restore();

    result
}
