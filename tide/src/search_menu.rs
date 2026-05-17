/*****************************************************
 * Copyright 2026, Tide Project
 *****************************************************/

/*****************************************************
 * Crates
 *****************************************************/

use std::{sync::mpsc::Receiver, path::Path};

use crate::{application::Direction};
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

    pub fn add_field (&mut self, item : SearchItem) {
        self.popup = self.popup.clone().add_field(item);       
    }

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

    pub fn search (&mut self, cwd : &Path) -> Receiver<(u32, SearchItem)> {
        
        self.popup.reset();

        let query= self.input.get_input_to_render();
        
        search::search(cwd,query) 
    }

    pub fn handle_event(&mut self, event : &Event)
    {
        self.input.handle_event(event);
    }
}

