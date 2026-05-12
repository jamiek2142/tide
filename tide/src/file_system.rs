/*****************************************************
 * Copyright 2026, Tide Project
 *****************************************************/

/*****************************************************
 * Crates 
 *****************************************************/

use crate::application::Direction;
use crate::shell::Shell;

use std::path::{Path, PathBuf};
use std::slice::Iter;
use std::cell::RefCell;
use std::rc::Rc;

use ratatui::widgets::{ListState};

use walkdir::{DirEntry, WalkDir};

use std::cmp::Ordering;

/*****************************************************
 * Types
 *****************************************************/

#[derive(Default, Clone)]
pub struct FileEntry { 
    path     : PathBuf,  
    basename : String,   
    depth    : usize,    
    is_dir   : bool,
    expanded : bool, 
}

pub struct FileTree {
    current_path     : PathBuf,
    file_entries     : Vec<FileEntry>,
    list_state       : ListState,
    shell            : Rc<RefCell<Shell>>
}
 
/*****************************************************
 * Implementations
 *****************************************************/

fn is_dotfile(entry: &DirEntry) -> bool {
    // TODO: Enable/disable dotfiles.

    for component in entry.path().iter() {
        if component.to_string_lossy().starts_with(".") {
            return true;
        }
    }

    false
}

/*****************************************************
 * Implementations
 *****************************************************/

impl FileEntry {

    pub fn new (dir_entry : DirEntry, root_depth : usize) -> Self
    {
        Self { 
            path: dir_entry.path().to_path_buf(), 
            basename: dir_entry.file_name().to_string_lossy().to_string(), 
            depth: root_depth + dir_entry.depth(), 
            is_dir: dir_entry.path().is_dir(),
            expanded : false
        }
    }

    pub fn is_dir (&self) -> bool {
        self.is_dir
    }

    pub fn path (&self) -> String { 
        "  ".repeat(self.depth - 1) +  &self.basename
    }

}

impl FileTree {
  
    fn insert_entries (&mut self, path : &Path, index : Option<usize>, depth : Option<usize>) 
    { 
        let walker = WalkDir::new(path)
                                .max_depth(1)
                                .min_depth(1)
                                .sort_by(|a, b| {
            let a_is_dir = a.file_type().is_dir();
            let b_is_dir = b.file_type().is_dir();

            match (a_is_dir, b_is_dir) {
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                _ => a.file_name().cmp(b.file_name()),
            }
        });

        for entry in walker.into_iter().filter_map(|e| e.ok()) {

            if is_dotfile(&entry) {
                continue;
            }

            let index = if let Some(index) = index { index + 1 } else { 0 } ;
            let depth = if let Some(depth) = depth { depth } else { 0 } ;
            self.file_entries.insert(index, FileEntry::new(entry, depth));           
        }
    }


    fn remove_entries (&mut self, index : usize)
    {   
       let depth = self.file_entries[index].depth;
       let mut k = 0;
       self.file_entries.retain(|entry | {
            k = k + 1; 
            (entry.depth <= depth) || (k <= index)
         }); 
     }

    pub fn new (shell : Rc<RefCell<Shell>>) -> Self {
    
        Self { current_path: PathBuf::default(), file_entries: Vec::new(), list_state: ListState::default(), shell : shell }
    }
    
    /** Traverse directories, selecting files/folders. 
     *
     * \returns A file to open, none if this was a directory. 
     */
    pub fn traverse_dirs(&mut self, direction: Direction) -> Option<PathBuf> {
        let k = match direction {
            Direction::UP => match self.list_state.selected() {
                Some(k) => {
                    if k <= 0 {
                        self.file_entries.len() - 1
                    } else {
                        k - 1
                    }
                }
                None => 0,
            },
            Direction::DOWN => match self.list_state.selected() {
                Some(k) => {
                    if k >= self.file_entries.len() - 1 {
                        0
                    } else {
                        k + 1
                    }
                }
                None => 0,
            } 
        };

        self.list_state.select(Some(k));
        
        // Return path to file to open
        if self.file_entries[k].is_dir {
            None
        } else {
            Some(self.file_entries[k].path.clone())
        }
    }

    pub fn select_entry (&mut self, k : usize) -> Option<PathBuf> {
       
        if k < self.file_entries.len() {   
            self.list_state.select(Some(k));
        
            if self.file_entries[k].is_dir {
                
                self.toggle_dir();

                None
            } else {
                Some(self.file_entries[k].path.clone())
            }
        } else {
            None
        }
    }


    pub fn change_dir_at_index (&mut self, k : usize) {

        if k < self.file_entries.len() {

            if self.file_entries[k].is_dir {
                self.change_dir(self.file_entries[k].path.clone());
            }
        }
            
    }

    pub fn change_dir (&mut self, path : PathBuf) {
        self.file_entries.clear();
        
        self.file_entries.push(FileEntry { path : PathBuf::new(), basename: "..".to_string(), depth: 1, is_dir: true, expanded: false });
        self.list_state.select(Some(0));

        self.insert_entries(&path, Some(0), None);
        self.current_path = path;
    }

    pub fn toggle_dir (&mut self) -> bool {

        let Some(index) = self.list_state.selected() else {
            return false;
        };

        // Handle special case for index 0 to move up a directory. 
        if index == 0 {
           
            let target_path = { let mut path = self.current_path.clone(); path.push(".."); path }; 
            let target_path =  std::fs::canonicalize(&target_path).unwrap_or(self.current_path.clone());
            
            self.shell.borrow_mut().set_cwd(target_path.clone());
            self.change_dir(target_path);
        }

        if ! self.file_entries[index].is_dir {
            return false;
        }

        if self.file_entries[index].expanded {

            self.file_entries[index].expanded = false;

            self.remove_entries(index);
        } else {
            let path = self.file_entries[index].path.clone();
            let depth  = self.file_entries[index].depth;

            self.insert_entries(&path, Some(index), Some(depth));

            self.file_entries[index].expanded = true;
        }
        true
    }

    pub fn iter (&self) -> Iter<'_, FileEntry>
    {
        self.file_entries.iter()
    }

    pub fn get_state (&mut self) -> &mut ListState
    {
       &mut self.list_state
    }

    pub fn get_current_dir_to_render(& self) -> PathBuf
    {
        self.current_path.clone() 
    }

}

