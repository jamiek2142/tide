/*****************************************************
 * Copyright 2026, Tide Project
 *****************************************************/

/*****************************************************
 * Crates 
 *****************************************************/

use std::collections::VecDeque;

use crossterm::{Command, event::{Event, KeyCode}};

/*****************************************************
 * Types
 *****************************************************/

#[derive(Clone)]
pub struct Input
{
    command_queue   : VecDeque<String>,
    command_buffer  : Vec<String>,
    command_index   : usize,
    cursor_position : usize
}

/*****************************************************
 * Implementations
 *****************************************************/

impl Input
{
    pub fn new () -> Self 
    {
        Self 
        { command_queue   : VecDeque::new(), 
          command_buffer  : vec!["".to_string()], 
          command_index   : 0, 
          cursor_position : 0 
        }
    }

    pub fn pop_command (&mut self) -> Option<String>
    {
        self.command_queue.pop_front()
    }
    
    fn push_command (&mut self)
    {
        if self.command_index < self.command_buffer.len()
        {
            let command = self.command_buffer[self.command_index].clone();                   

            self.command_queue.push_back(command);
        }
    }

    pub fn get_input_to_render (&self) -> &str
    {
        if self.command_index >= self.command_buffer.len()
        {
            "" 
        }
        else 
        {
            self.command_buffer[self.command_index].as_str()
        }
    }
    
    pub fn handle_event(&mut self, event : &Event)
    {
        match event
        {
            Event::Key(key) => {
                match key.code {
                    KeyCode::Char(c) => {

                        if self.command_index < self.command_buffer.len()
                        {
                            self.command_buffer[self.command_index].insert(self.cursor_position, c);
                            self.cursor_position = self.cursor_position + 1;
                        }

                    },
                    KeyCode::Left => {
                        if self.cursor_position > 0
                        {
                            self.cursor_position = self.cursor_position - 1;
                        }
                    }
                    KeyCode::Right => {
                        if self.cursor_position < (self.command_buffer[self.command_index].len() - 1)
                        {
                            self.cursor_position = self.cursor_position + 1;
                        }   
                    },
                    KeyCode::Up => {
                        if self.command_index > 0
                        {
                            self.command_index = self.command_index - 1;
                        }
                    },
                    KeyCode::Down => {
                        if self.command_index < (self.command_buffer.len() - 1)
                        {
                            self.command_index = self.command_index + 1;
                        }
                    },
                    KeyCode::Enter => {
                        self.push_command();
                        
                        self.command_buffer.push("".to_string());
                        self.cursor_position = 0;
                        self.command_index   = self.command_index + 1;
                    },
                    KeyCode::Backspace => { 
                        if self.cursor_position > 0
                        {
                            if self.command_index < self.command_buffer.len()
                            {
                                self.command_buffer[self.command_index].remove(self.cursor_position);
                                self.cursor_position = self.cursor_position - 1;
                            }
                        }
                    },
                    _ => {
                        /* TODO: Handle other KeyCodes. */
                    }
                }
            },
            _ => {
            },
        }
    }
        
}


