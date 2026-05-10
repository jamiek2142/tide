/*****************************************************
 * Copyright 2026, Tide Project
 *****************************************************/

/**
 * This file implements the main application logic.
 */

/*****************************************************
 * Crates
 *****************************************************/

use crate::popup_menu::{self, PopupMenu};
use crate::{file_system, input::Input};
use crate::shell::Shell;
use crate::file_system::FileTree;

use std::env::current_exe;
use std::{
    cmp::Ordering, 
    collections::HashMap, 
    default, 
    env, 
    ffi::OsString, 
    fs, 
    io, 
    path::{
        Path,
        PathBuf
    }, 
    process::Command, 
    thread::sleep, 
    time::Duration, 
    cell::RefCell, 
    rc::Rc
};

use color_eyre::owo_colors::colors::Default;

use crossterm::{cursor, event, event::{Event, KeyCode, KeyEvent, KeyEventKind}};
use crossbeam_channel::{Receiver, Sender, unbounded};

use portable_pty::{CommandBuilder, NativePtySystem, PtyPair, PtySize, PtySystem};

use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Margin, Constraint, Layout, Position, Rect, Spacing},
    style::{Color, Modifier, Style, Stylize},
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Widget},
    symbols::merge::MergeStrategy
};

use ratatui_textarea::{
    TextArea
};

use ratatui_code_editor::{
    editor::{ Editor
    }, theme::vesper
};

use ansi_to_tui::IntoText as _;

/*****************************************************
 * Types
 *****************************************************/

#[derive(PartialEq)]
enum EditorFocus {
    MAIN,
    EXIT
}

#[derive(PartialEq)]
enum Focus {
    FILES,
    SHELL,
    EDITOR(EditorFocus)
}


pub enum Direction {
    UP,
    DOWN,
}
pub struct App {
    input : Input,
    file_system : FileTree,
    shell : Rc<RefCell<Shell>>,
    exit : bool,
    output : Vec<String>,
    rx : Receiver<Vec<u8>>,
    focus : Focus,
    editor : Option<Editor>,
    open_file : Option<PathBuf>,
    popup_menu : Option<PopupMenu> 
}

/*****************************************************
 * Types
 *****************************************************/

impl From<KeyCode> for Direction {

    fn from(value: KeyCode) -> Self {
       match value {
         KeyCode::Up => Direction::UP,
         _ => Direction::DOWN
       }
    }

}

impl App {
    pub fn new() -> Self {
        let (tx, rx) = unbounded::<Vec<u8>>();
        
        let shell = Rc::new(RefCell::new(Shell::new(tx)));   

        Self {
            input: Input::new(),
            file_system: FileTree::new(Rc::clone(&shell)),
            shell: shell,
            exit: bool::default(),
            output: Vec::new(),
            rx: rx,
            focus: Focus::FILES,
            editor: None,
            open_file : None,
            popup_menu : None
       }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
      
        let target_path = self.shell.borrow().cwd_as_path();
        self.change_dir(&target_path);

        while !self.exit {
            while let Ok(bytes) = self.rx.try_recv() {
                let text = String::from_utf8_lossy(&bytes).to_string();
                let mut text = text.lines().map(String::from).collect();
                self.output.append(&mut text);
            }

            terminal.draw(|frame| self.draw(frame))?;

            if let Some(command) = self.input.pop_command() { 
                self.execute(command);  
            };

         }
        Ok(())
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    fn draw(&mut self, frame: &mut Frame) {

        let [file_area, main_area] = 
            Layout::horizontal([
                Constraint::Percentage(30), 
                Constraint::Percentage(70)
            ]).spacing(Spacing::Overlap(1)).split(frame.area())[..] 
            else { 
                todo!() 
            };
        let [editor_area, shell_output, shell_input] = 
            Layout::vertical([
                Constraint::Fill(24), 
                Constraint::Min(10), 
                Constraint::Min(1)
            ]).split(main_area)[..] 
            else {
                 todo!() 
            };
        
        let editor_area = editor_area.inner(Margin::new(1, 0));
    
        let text = " > ".to_string() + self.input.get_input_to_render();

        let input_block = {
               
                let block = Block::new()
                    .borders(Borders::LEFT)
                    .merge_borders(MergeStrategy::Exact);

                match self.focus {      
                    Focus::SHELL => {
                        block.border_style(Style::new().light_green())
                    }      
                    _ => {
                        block
                    }
                }
            };

        let input = Paragraph::new(text)
            .style(Style::default())
            .block(input_block);

    
        let num_lines = { 
                self.output
                    .len()
                    .saturating_sub(
                        (shell_output.height as usize)
                            .saturating_sub(1)
                        )
            };
        let last_lines = &self.output[num_lines..];

        let text: Text = last_lines
                        .join("\n")
            .into_bytes()
            .into_text()
            .unwrap_or_default();

        let output_block = {
               
                let block = Block::new()
                    .borders(Borders::LEFT | Borders::TOP)
                    .merge_borders(MergeStrategy::Exact);
                
                match self.focus {      
                    Focus::SHELL => {
                        block.border_style(Style::new().light_green())
                    }      
                    _ => {
                        block
                    }
                }
            };

        let output = Paragraph::new(text)
            .style(Style::default())
            .block(output_block);

        let items: Vec<ListItem> = self.file_system
            .iter()
            .map(|k| {
                let style = if k.is_dir() {
                    Style::default()
                        .fg(Color::LightMagenta)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(k.path()).style(style)
            })
            .collect();

        let files_block = {
                let block = Block::new()
                    .borders(Borders::RIGHT)                    
                    .merge_borders(MergeStrategy::Exact);
               
                match self.focus {
                    Focus::FILES => {
                        block.border_style(Style::new().light_green())
                    }
                    _ => {
                        block
                    } 
                }
            }; 

            
        let mut current_dir_path = self.file_system.get_current_dir_to_render();
        let avaiable_space = if file_area.width > 15 { 
                15 
            } else { 
                file_area.width 
            };
        let mut required_space = 0; 
        let mut num_elems = 0;

        for elem in current_dir_path.iter().rev() {
            let num_chars : u16 = elem.to_str().unwrap().chars().count().try_into().unwrap();

            required_space = required_space + num_chars;
        
            if required_space > avaiable_space 
            {
                break;
            }
            
            num_elems = num_elems + 1;
        };

        let current_dir_path = self.file_system.get_current_dir_to_render();
        let num_components    = current_dir_path.components().count();
            
        let dir_path_to_render = if num_elems < num_components {
                let dir_path_to_render : PathBuf = current_dir_path.components().skip(num_components - num_elems - 1).collect();

                "../".to_string() + &dir_path_to_render.to_string_lossy()
            } else {
                current_dir_path.to_string_lossy().to_string()
            };

        let list = List::new(items)
            .block(
                files_block
                    .title(dir_path_to_render)
                    .title_style(
                        Style::default()
                            .fg(Color::LightMagenta)
                            .add_modifier(Modifier::BOLD)
                        ),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Yellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");
 
        // Render the fils and shell input. Set the cursor position if the shell is active focus. 
        frame.render_stateful_widget(list, file_area, &mut self.file_system.get_state());
        frame.render_widget(input, shell_input);

        if self.focus == Focus::SHELL { 

           let cursor_offset_x = 4 + shell_input.x +  self.input.get_cursor_position();
           let cursor_offset_y = shell_input.y; 

           frame.set_cursor_position((cursor_offset_x, cursor_offset_y));
        
        }
 
        // Render text editor and shell output
        match &self.editor {
            Some (editor) => {
                frame.render_widget(editor, editor_area);
           
                let cursor = editor.get_visible_cursor(&editor_area);

                if let Some((x,y)) = cursor {
                    frame.set_cursor_position(Position::new(x,y));
                }
            }, 
            None => {  
            }
        }
        
        frame.render_widget(output, shell_output); 

        // Render the popup menu if set.
        if let Some(popup) = &mut self.popup_menu {

            let popup_area_width = editor_area.width / 4;
            let popup_area_height = 10;

            let popup_area_x = editor_area.x + (editor_area.width - popup_area_width) / 2 ;
            let popup_area_y = editor_area.y + (editor_area.height - popup_area_height) / 2 ;

            let popup_area = Rect::new(popup_area_x, popup_area_y, popup_area_width, popup_area_height);
            let popup_block = Block::default().borders(Borders::ALL);


            let popup_items = popup.get_list_items();
            let popup_items : Vec<ListItem> = popup_items
                .iter()
                .map(|k| {
                    ListItem::new(k.as_str())
                })
                .collect();

            let popup_list = List::new(popup_items)
                .block(popup_block)
                .highlight_style(
                    Style::default()
                        .bg(Color::Yellow)
                        .fg(Color::Black)
                    .   add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> ");

            frame.render_widget(Clear, popup_area);

            frame.render_stateful_widget(popup_list, popup_area, popup.get_state());
        }


        self.handle_events(&editor_area).unwrap();
    }

    fn handle_events(&mut self, editor_area : &Rect) -> io::Result<()> {
        if event::poll(Duration::from_millis(10))? {
            match event::read()? {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    self.handle_key_event(key_event, editor_area)
                }
                Event::Mouse(mouse_event) => {
                    match &mut self.editor {
                        Some(editor) => {                        
                            editor.mouse(mouse_event, editor_area);
                        },
                        None => {
                            /* Nothing to do. */
                        },
                    }
                }
                _ => { /* Nothing to do. */ }
            }
        }
        Ok(())
    }

    fn change_dir(&mut self, target_path: &PathBuf) {
       
        // TODO: Handle invalid paths. 
        
        let target_path = std::fs::canonicalize(&target_path).unwrap_or(self.shell.borrow_mut().cwd_as_path());

        self.shell.borrow_mut().set_cwd(target_path.clone());

        self.file_system.change_dir(target_path);
    }

    fn open_file(&mut self, target_path: &PathBuf) {
        
        let content = if target_path.exists() {
            match fs::read_to_string(target_path) {
                Ok(ok)  => ok,
                Err(err) => return
            }
        } else {
            return;
        };

        let editor = Editor::new("rust", content.as_str(), vesper()).unwrap();
       
        self.open_file = Some(target_path.clone());
        self.editor    = Some(editor);
    }

    fn handle_file_key_press (&mut self) {

        if ! self.file_system.toggle_dir() {
            self.focus = Focus::EDITOR(EditorFocus::MAIN);
        }
    }

    fn execute(&mut self, command : String) {
        let argv: Vec<&str> = command.trim()
                                .split_whitespace()
                                .collect();

        if argv.len() == 0 {
            return;         
        }

        match argv[0] {

            "clear" => {
                self.output.clear();
            },
            "cd" => {
                if argv.len() > 1 {
                    let path_arg = PathBuf::from(argv[1]);

                    let target_path = if path_arg.is_absolute() {
                        path_arg
                    } else {
                        self.shell.borrow_mut().get_cwd().join(path_arg)
                    };
                    if target_path.is_dir() {
                        self.change_dir(&target_path);
                    }
                    // TODO: Print invalid directory.
                }
            }

            _ => {
                self.shell.borrow_mut().send_cmd(argv);
            }
        }

    }

    fn clear_output(&mut self) {
        self.output.clear();
    }

    fn handle_key_event(&mut self, key_event: KeyEvent, editor_area : &Rect) {
        match key_event.code {
            KeyCode::Esc   => {
                match &self.focus {
                    Focus::SHELL |
                    Focus::FILES  => self.exit(),
                    Focus::EDITOR(editor_focus) => {

                        match editor_focus {

                            EditorFocus::MAIN => {
                                self.popup_menu = Some(PopupMenu::default().add_field("Save?").add_field("Exit?"));
                                self.focus      = Focus::EDITOR(EditorFocus::EXIT);
                            },

                            EditorFocus::EXIT => {   

                                self.popup_menu = None;
                                self.focus      = Focus::EDITOR(EditorFocus::MAIN); 
                            }
                        }
                    },
                }
            },
            KeyCode::Down | KeyCode::Up  => {
                match &self.focus {
                  Focus::FILES  => {
                      if let Some(file) = self.file_system.traverse_dirs(Direction::from(key_event.code)) {
                        self.open_file(&file);
                      }
                  },
                  Focus::SHELL  => {
                        self.input.handle_event(&Event::Key(key_event));
                  },
                  Focus::EDITOR(editor_focus) => {
                        match editor_focus {
                            EditorFocus::MAIN => {
                                if let Some(editor) = &mut self.editor {
                                    editor.input(key_event, editor_area);
                                };
                            },
                            EditorFocus::EXIT => {
                                if let Some(popup) = &mut self.popup_menu {
                                    popup.traverse_items(Direction::from(key_event.code));
                                };
                            },
                        }
                    },
                }
            }, 
            
            KeyCode::Modifier(modifiier) => {

                match self.focus {
                    Focus::FILES => {
                        // TODO: File modifiiers.
                    },
                    Focus::SHELL => {
                        self.input.handle_event(&Event::Key(key_event));
                    },
                    Focus::EDITOR(_) => {
                        match &mut self.editor {
                            Some(editor) => {
                                editor.input(key_event, editor_area);
                            },
                            None => {
                                /* Nothing to do */
                            }          
                        }
                    }
                }
            },

            KeyCode::Tab => {
                
                match self.focus {
                    Focus::FILES     => {
                        if let Some(editor) = &self.editor {
                            self.focus = Focus::EDITOR(EditorFocus::MAIN);
                        } else {
                            self.focus = Focus::SHELL;
                        };
                    }, 
                    Focus::EDITOR(_) => self.focus = Focus::SHELL,
                    Focus::SHELL     => self.focus = Focus::FILES,
                }
            },

            KeyCode::Enter => {
                
                match &self.focus {
                    Focus::FILES  => {
                        self.handle_file_key_press(); 
                    },
                    Focus::SHELL  => {
                        self.input.handle_event(&Event::Key(key_event));
                    },
                    Focus::EDITOR(editor_focus) => {
                        match editor_focus {
                            EditorFocus::MAIN => {
                                if let Some(editor) = &mut self.editor {
                                    editor.input(key_event, editor_area);
                                };
                            },
                            EditorFocus::EXIT => { 
                                let mut close_menu = false;
                                if let Some(popup_menu) = &mut self.popup_menu {

                                    if popup_menu.selected("Save?") {

                                        let content = self.editor
                                                        .as_ref()
                                                        .unwrap()
                                                        .get_content();

                                        fs::write(self.open_file.as_ref().unwrap(), content);
                                    
                                    }

                                    if popup_menu.selected("Exit?") {
                                        self.editor = None;

                                        self.focus  = Focus::FILES;

                                        close_menu  = true;     
                                    }
                                };

                                if close_menu {
                                    self.popup_menu = None;
                                }
                            },
                        }
                    },
                }
            },
            _ => {
                match self.focus {
                    Focus::FILES => {
                        // TODO: Handle other keys
                    },  
                    Focus::SHELL => {
                        self.input.handle_event(&Event::Key(key_event));
                    },
                    Focus::EDITOR(_) =>  {
                        match &mut self.editor {
                            Some(editor) => { 
                                editor.input(key_event,editor_area);
                            },
                            None => {
                                /* Nothing to do */
                            },
                        }
                    },
                }
            }
        }
    }
}

