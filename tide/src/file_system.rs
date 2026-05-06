/*****************************************************
 * Copyright 2026, Tide Project
 *****************************************************/

/*****************************************************
 * Crates 
 *****************************************************/

use std::path::PathBuf;
use std::slice::Iter;

use ratatui::widgets::ListState;

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

/*****************************************************
 * Implementations
 *****************************************************/

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

