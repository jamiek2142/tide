/*****************************************************
 * Copyright 2026, Tide Project
 *****************************************************/

/*****************************************************
 * Crates 
 *****************************************************/

use crossterm::event::{Event, KeyCode};

/*****************************************************
 * Types
 *****************************************************/

#[derive(Default, Clone)]
pub struct Input
{
    value : String
}

/*****************************************************
 * Implementations
 *****************************************************/

impl Input
{
    pub fn reset (&mut self)
    {
        self.value.clear();
    }

    pub fn value (&self) -> &str
    {
        self.value.as_str()
    }

    pub fn handle_event(&mut self, event : &Event)
    {
        match event
        {
            Event::Key(key) => {
                match key.code {
                    KeyCode::Char(c) => self.value.push(c),
                    KeyCode::Backspace => { self.value.pop(); }
                    _ => {

                    }
                }
            },
            _ => {
            },
        }
    }
        
}


