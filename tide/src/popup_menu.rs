/*****************************************************
 * Copyright 2026, Tide Project
 *****************************************************/

/*****************************************************
 * Crates
 *****************************************************/

use crate::application::Direction;

use ratatui::widgets::{ListItem, ListState};

/*****************************************************
 * Types
 *****************************************************/

#[derive(Default, Clone)]
pub struct PopupMenu {
    list  : Vec<String>,
    state : ListState
}

/*****************************************************
 * Implementations
 *****************************************************/

impl PopupMenu {

   /** Add a single field to a PopupMenu.
    *
    * \param[in] text   Text to add for the specific field.
    *
    * \returns The popup menu object for method chaining.
    */
   pub fn add_field(mut self, text :  &str) -> Self {  

        let was_empty = self.list.is_empty();

        self.list.push(text.to_string()); 
       
        if was_empty {
            self.state.select(Some(0));
        }

        self
   }

   pub fn get_state(&mut self) -> &mut ListState {
        &mut self.state
   }

   pub fn get_list_items (& self) -> Vec<String> {
        self.list.clone()
   }

   pub fn selected (&self, text : &str) -> bool {
        
       let Some(index) = self.state.selected() else {
           return false;
       };

       if index >= self.list.len() {
           return false;
       }

       self.list[index] == text
   }
    
   pub fn traverse_items (&mut self, direction: Direction) {
        let k = match direction {
            Direction::UP => match self.state.selected() {
                Some(k) => {
                    if k <= 0 {
                        self.list.len() - 1
                    } else {
                        k - 1
                    }
                }
                None => 0,
            },
            Direction::DOWN => match self.state.selected() {
                Some(k) => {
                    if k >= self.list.len() - 1 {
                        0
                    } else {
                        k + 1
                    }
                }
                None => 0,
            } 
        };

        self.state.select(Some(k));
       
    }
}

