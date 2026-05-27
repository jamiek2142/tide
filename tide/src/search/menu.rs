/*****************************************************
 * Copyright 2026, Tide Project
 *****************************************************/

/*****************************************************
 * Crates
 *****************************************************/

use std::{path::Path, sync::mpsc::Receiver};

use crate::application::Direction;
use crate::input::Input;
use crate::popup_menu::PopupMenu;

use crate::search;
use crate::search::{SearchHandle, SearchItem};
use ratatui::widgets::ListState;

use crossterm::event::Event;

/*****************************************************
 * Types
 *****************************************************/

#[derive(Default)]
pub struct SearchMenu {
    input: Input,
    popup: PopupMenu<(u32, SearchItem)>,
    handle: Option<SearchHandle>,
    n_items: usize,
}

/*****************************************************
 * Implementations
 *****************************************************/

impl SearchMenu {
    pub fn add_field(&mut self, score: u32, item: SearchItem) {
        self.n_items += 1;

        let index = match self
            .popup
            .get_list_items()
            .binary_search_by(|elem| elem.0.cmp(&score).reverse())
        {
            Ok(index) => index,
            Err(index) => index,
        };

        self.popup.insert_field(index, (score, item), Some(1000));
    }

    pub fn get_input_to_render(&self) -> &str {
        self.input.get_input_to_render()
    }

    pub fn get_state(&mut self) -> &mut ListState {
        self.popup.get_state()
    }

    pub fn get_list_items(&self) -> Vec<SearchItem> {
        self.popup
            .get_list_items()
            .iter()
            .map(|(_, item)| item.clone())
            .collect()
    }

    pub fn get_selected_item(&self) -> Option<&SearchItem> {
        self.popup.get_selected().map(|(_, item)| item)
    }

    pub fn traverse_items(&mut self, direction: Direction) {
        self.popup.traverse_items(direction)
    }

    pub fn get_rx(&mut self) -> Option<&mut Receiver<(u32, SearchItem)>> {
        if let Some(handle) = &mut self.handle {
            Some(&mut handle.rx)
        } else {
            None
        }
    }

    pub fn running(&self) -> bool {
        if let Some(handle) = &self.handle {
            !(handle.t1.is_finished() && handle.t2.is_finished())
        } else {
            false
        }
    }

    pub fn get_n_items(&self) -> usize {
        self.n_items
    }

    pub fn cleanup(&mut self) {
        self.popup.reset();
        self.n_items = 0;

        if let Some(handle) = self.handle.take() {
            handle.t1.join().unwrap();
            handle.t2.join().unwrap();
        }
    }

    pub fn search(&mut self, cwd: &Path) {
        if self.running() {
            return;
        }
        self.cleanup();

        let query = self.input.get_input_to_render();

        self.handle = Some(search::search(cwd, query));
    }

    pub fn handle_event(&mut self, event: &Event) {
        if self.running() {
            return;
        }

        self.input.handle_event(event);
    }
}
