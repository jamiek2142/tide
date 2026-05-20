/*****************************************************
 * Copyright 2026, Tide Project
 *****************************************************/

/*****************************************************
 * Crates
 *****************************************************/

use crate::application::Direction;

use ratatui::widgets::{ListState};

/*****************************************************
 * Types
 *****************************************************/

#[derive(Default, Clone)]
pub struct PopupMenu<T> where T : Clone + PartialEq {
    list  : Vec<T>,
    state : ListState
}

/*****************************************************
 * Implementations
 *****************************************************/

impl<T> PopupMenu<T> where T : Clone + PartialEq {

   /** Add a single field to a PopupMenu.
    *
    * \param[in] text   Text to add for the specific field.
    *
    * \returns The popup menu object for method chaining.
    */
   pub fn add_field(mut self, field : T) -> Self {  

        let was_empty = self.list.is_empty();

        self.list.push(field); 
       
        if was_empty {
            self.state.select(Some(0));
        }

        self
   }

    pub fn insert_field(&mut self, index : usize, field : T, max_count : Option<usize>) {
        
        let was_empty = self.list.is_empty();

        self.list.insert(index, field); 
      
        if let Some(max_count) = max_count {
            self.list.truncate(max_count);

            if let Some(index) = self.state.selected() {
 
                if index >= self.list.len() {
                    self.state.select(Some(self.list.len() - 1));
                }
            }
        };

        if was_empty {
            self.state.select(Some(0));
        }

    }

   pub fn reset (&mut self) {
        self.list.clear();
        self.state.select(None);
   }

   pub fn get_state(&mut self) -> &mut ListState {
        &mut self.state
   }

   pub fn get_list_items (& self) -> &[T] {
        &self.list
   }

   pub fn get_selected (&self) -> Option<&T> {
        
        let Some(index) = self.state.selected() else {
            return None;
        };

        Some(&self.list[index])
   }

   pub fn selected (&self, field : T) -> bool {
        
       let Some(index) = self.state.selected() else {
           return false;
       };

       if index >= self.list.len() {
           return false;
       }

       self.list[index] == field
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

