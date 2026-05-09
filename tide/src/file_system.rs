/*****************************************************
 * Copyright 2026, Tide Project
 *****************************************************/

/*****************************************************
 * Crates 
 *****************************************************/

use crate::application::Direction;

use std::env::current_exe;
use std::path::{Path, PathBuf};
use std::slice::Iter;

use ratatui::widgets::ListState;

use walkdir::{DirEntry, WalkDir};

use std::cmp::Ordering;

/*****************************************************
 * Types
 *****************************************************/

#[derive(Default, Clone)]
pub struct FilePath {
    path: String,
    is_dir: bool,
}


#[derive(Default, Clone)]
pub struct FileSystem {
    current_dir_to_render: String,
    paths_to_render: Vec<FilePath>,
    paths_to_objects: Vec<PathBuf>,
    state : ListState
}

#[derive(Default, Clone)]
pub struct FileEntry { 
    path     : PathBuf,  
    basename : String,   
    depth    : usize,    
    is_dir   : bool,      
}

#[derive(Default, Clone)]
pub struct FileTree {
    current_path     : PathBuf,
    file_entries     : Vec<FileEntry>,
    list_state       : ListState
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
            is_dir: dir_entry.path().is_dir() 
        }
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
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                _ => a.file_name().cmp(b.file_name()),
            }
        });

        for entry in walker.into_iter().filter_map(|e| e.ok()) {

            let index = if let Some(index) = index { index + 1 } else { 0 } ;
            let depth = if let Some(depth) = depth { depth } else { 0 } ;
            self.file_entries.insert(index, FileEntry::new(entry, depth));           
        }
    }

    /** Traverse directories, selecting files/folders. 
     *
     * \returns True if the currently selected entry is a directory.
     */
    fn traverse_dirs(&mut self, direction: Direction) -> bool {
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
        
        self.file_entries[k].is_dir
    }

    pub fn change_dir (&mut self, path : PathBuf) 
    {
        self.file_entries.clear();

        self.insert_entries(&path, None, None);

        self.current_path = path;
    }

    pub fn expand_dir (&mut self) -> bool {

        let Some(index) = self.list_state.selected() else {
            return false;
        };

        if ! self.file_entries[index].is_dir {
            return false;
        }

        let path = self.file_entries[index].path.clone();
        let depth  = self.file_entries[index].depth;

        self.insert_entries(&path, Some(index), Some(depth));

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

}

impl FilePath {
    pub fn new(path: String, is_dir: bool) -> Self {
        let path = if is_dir { path + "/" } else { path };

        Self {
            path: path,
            is_dir: is_dir
        }
    }

    pub fn is_dir (& self) -> bool
    {
        self.is_dir
    }

    pub fn path (& self) -> &str
    {
        self.path.as_str()
    }
}

impl FileSystem {
    pub fn clear(&mut self) {
        self.paths_to_render.clear();
        self.paths_to_objects.clear();
    }

    pub fn len (& self) -> usize
    {
        self.paths_to_render.len()
    }

    pub fn get_state (&mut self) -> &mut ListState
    {
       &mut self.state
    }

    pub fn push_path_to_object (&mut self, path : PathBuf)
    {
        self.paths_to_objects.push(path);
    }

    pub fn get_path_to_object (& self, index : usize) -> PathBuf
    {
        self.paths_to_objects[index].clone()
    }

    pub fn push_path_to_render (&mut self, path : &str, is_dir : bool)
    {
        self.paths_to_render.push(FilePath::new(path.to_string(), is_dir));
    }

    pub fn set_current_dir_to_render(&mut self, path : &str) 
    {
        self.current_dir_to_render = path.to_string();
    }

    pub fn get_current_dir_to_render(& self) -> &str
    {
        self.current_dir_to_render.as_str()
    }

    pub fn get_paths_to_render (&self) -> Iter<'_, FilePath>
    {
        self.paths_to_render.iter()
    }
}

