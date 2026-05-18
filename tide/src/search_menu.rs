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

use crate::search::{SearchItem, SearchHandle};
use crate::search;
use ratatui::widgets::{ListState};

use crossterm::event::Event;

/*****************************************************
 * Types
 *****************************************************/

#[derive(Default)]
pub struct SearchMenu {
    input   : Input,
    popup   : PopupMenu<(u32, SearchItem)>,
    handle  : Option<SearchHandle>
}

/*****************************************************
 * Implementations
 *****************************************************/

impl SearchMenu {

    pub fn add_field (&mut self, score : u32, item : SearchItem) {
        
        let index = match self.popup
            .get_list_items()
            .binary_search_by(|elem| elem.0.cmp(&score).reverse()) {
            Ok(index) => index,
            Err(index) => index
            
        };

        self.popup.insert_field(index, (score, item)); 
    }

    pub fn get_input_to_render (&self) -> &str {
        self.input.get_input_to_render()
    }

    pub fn get_state(&mut self) -> &mut ListState {
        self.popup.get_state()
    }

    pub fn get_list_items (& self) -> Vec<SearchItem> {
        self.popup.get_list_items().iter().map(|(_, item)| item.clone()).collect()
     }
    
    pub fn traverse_items (&mut self, direction: Direction) {
        self.popup.traverse_items(direction)       
    }

    pub fn get_rx (&mut self) -> Option<&mut Receiver<(u32, SearchItem)>> {
        if let Some(handle) = &mut self.handle { Some(&mut handle.rx) } else { None } 
    }

    pub fn cleanup (&mut self) {
        
        self.popup.reset();

        if let Some(handle) = self.handle.take() {
            let _ = handle.t1.join().unwrap();
            let _ = handle.t2.join().unwrap();
        }

    }

    pub fn search (&mut self, cwd : &Path) {
        
        self.cleanup();
        
        let query= self.input.get_input_to_render();
        
        self.handle = Some(search::search(cwd,query));
    }

    pub fn handle_event(&mut self, event : &Event)
    {
        self.input.handle_event(event);
    }
}

