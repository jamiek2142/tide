/*****************************************************
 * Copyright 2026, Tide Project
 *****************************************************/

/*****************************************************
 * Crates
 *****************************************************/

use std::path::Path;

use crate::application::Direction;
use crate::input::Input;
use crate::popup_menu::PopupMenu;

use crate::search::SearchItem;
use crate::search;

use ratatui::widgets::{ListState};

use crossterm::event::Event;

/*****************************************************
 * Types
 *****************************************************/

#[derive(Default, Clone)]
pub struct SearchMenu {
    input : Input,
    popup : PopupMenu<SearchItem>
}

/*****************************************************
 * Implementations
 *****************************************************/

impl SearchMenu {

    pub fn get_input_to_render (&self) -> &str {
        self.input.get_input_to_render()
    }

    pub fn get_state(&mut self) -> &mut ListState {
        self.popup.get_state()
    }

    pub fn get_list_items (& self) -> Vec<SearchItem> {
        self.popup.get_list_items()
     }
    
    pub fn traverse_items (&mut self, direction: Direction) {
        self.popup.traverse_items(direction)       
    }

    pub fn search (&mut self, cwd : &Path) {
        
        self.popup.reset();

        let query= self.input.get_input_to_render();
        
        let matches= search::search(cwd,query);
        
        self.popup = matches.iter()
                            .fold(self.popup.clone(), 
                            |acc, x| acc.add_field(x.clone()));
       
    }

    pub fn handle_event(&mut self, event : &Event)
    {
        self.input.handle_event(event);
    }
}

