/*****************************************************
 * Copyright 2026, Tide Project
 *****************************************************/

/*****************************************************
 * Crates
 *****************************************************/

use crate::application::Direction;
use crate::input::Input;
use crate::popup_menu::PopupMenu;

use crate::search::{
    self,
    SearchItem
};

use ratatui::widgets::{ListState};

use crossterm::event::Event;

/*****************************************************
 * Types
 *****************************************************/

#[derive(Default, Clone)]
pub struct SearchMenu {
    input : Input,
    popup : PopupMenu
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

    pub fn get_list_items (& self) -> Vec<String> {
        self.popup.get_list_items()
     }

    pub fn selected (&self, text : &str) -> bool {
        self.popup.selected(text)        
    }
    
    pub fn traverse_items (&mut self, direction: Direction) {
        self.popup.traverse_items(direction)       
    }

    pub fn search (&mut self) {
        
        self.popup.reset();

        let query= self.input.get_input_to_render();
        
        let matches= search::search(query);
        
        self.popup = matches.iter()
                            .fold(self.popup.clone(), 
                            |acc, x| acc.add_field(x.display()));
       
    }

    pub fn handle_event(&mut self, event : &Event)
    {
        self.input.handle_event(event);
    }
}

