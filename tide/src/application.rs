/*****************************************************
 * Copyright 2026, Tide Project
 *****************************************************/

/*****************************************************
 * Crates
 *****************************************************/

use crate::input::Input;
use crate::shell::Shell;
use crate::file_system::{FilePath,FileSystem};

use std::{
    cmp::Ordering, collections::HashMap, default, env, ffi::OsString, fs, io, path::{Path, PathBuf}, process::Command, thread::sleep, time::Duration, cell::Cell
};

use color_eyre::owo_colors::colors::Default;

use walkdir::{DirEntry, WalkDir};

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
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Widget},
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

#[derive(PartialEq)]
enum Focus {
    FILES,
    SHELL,
    EDITOR
}

enum Direction {
    UP,
    DOWN,
}
pub struct App {
    input : Input,
    file_system : FileSystem,
    shell : Shell,
    exit : bool,
    output : Vec<String>,
    tx : Sender<Vec<u8>>,
    rx : Receiver<Vec<u8>>,
    focus : Focus,
    editor : Option<Editor>,
    open_file : Option<PathBuf>
}
impl App {
    pub fn new() -> Self {
        let (tx, rx) = unbounded::<Vec<u8>>();

        Self {
            input: Input::default(),
            file_system: FileSystem::default(),
            shell: Shell::new(),
            exit: bool::default(),
            output: Vec::new(),
            tx: tx,
            rx: rx,
            focus: Focus::FILES,
            editor: None,
            open_file : None
       }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        self.update_file_system();

        while !self.exit {
            while let Ok(bytes) = self.rx.try_recv() {
                let text = String::from_utf8_lossy(&bytes);

                self.output.push(text.to_string());
            }

            terminal.draw(|frame| self.draw(frame))?;

         }
        Ok(())
    }

    fn send_cmd(&mut self, argv: Vec<&str>) {
        /* Clear the output pane. */
        self.clear_output();

        /* Create a PTY each command. */
        let pty_system = NativePtySystem::default();
        let pair = pty_system.openpty(PtySize::default()).unwrap();

        let argv = argv.into_iter().map(OsString::from).collect();

        let mut cmd = CommandBuilder::from_argv(argv);
        cmd.cwd(self.shell.get_cwd());
        // TODO: Environment variables.

        let Ok(mut _child) = pair.slave.spawn_command(cmd) else {
            self.output.push("Unknown command".to_string());
            // TODO: Optional clear input.
            return;
        };
        let mut reader = pair.master.try_clone_reader().unwrap();

        let tx = self.tx.clone();

        std::thread::spawn(move || {
            let mut buffer = [0u8; 1024];

            while let Ok(n) = reader.read(&mut buffer) {
                if n == 0 {
                    break;
                }
                let _ = tx.send(buffer[..n].to_vec());
            }
        });

        /* Clean out the command buffer */
        self.clear_input();
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
    
        let text = " > ".to_string() + self.input.value();

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

        let text: Text = self
            .output
            .clone()
            .join("")
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

        let files = self.file_system.clone();
        let items: Vec<ListItem> = files
            .get_paths_to_render()
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

        let list = List::new(items)
            .block(
                files_block.title(files.get_current_dir_to_render()),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Yellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");
      
        frame.render_stateful_widget(list, file_area, &mut self.file_system.get_state());
        frame.render_widget(input, shell_input);

        // Render output or text editor
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

    fn is_dotfile(&self, entry: &DirEntry) -> bool {
        // TODO: Enable/disable dotfiles.

        for component in entry.path().iter() {
            if component.to_string_lossy().starts_with(".") {
                return true;
            }
        }

        false
    }

    fn update_file_system(&mut self) {
        self.file_system.clear();
        self.file_system.get_state().select(Some(0));

        let walker = WalkDir::new(self.shell.get_cwd()).sort_by(|a, b| {
            let a_is_dir = a.file_type().is_dir();
            let b_is_dir = b.file_type().is_dir();

            match (a_is_dir, b_is_dir) {
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                _ => a.file_name().cmp(b.file_name()),
            }
        });

        let mut path_to_upper_dir = self.shell.cwd_as_path().clone();

        self.file_system.set_current_dir_to_render(&self.shell.get_cwd().to_string_lossy());

        path_to_upper_dir.push("..");

        let parent_dir = self
            .shell
            .get_cwd()
            .parent()
            .map_or(String::default(), |x| {
                x.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            });

        // TODO: Fix for root paths.
        self.file_system.set_current_dir_to_render(&("../".to_string()
            + &parent_dir
            + "/"
            + &self
                .shell
                .get_cwd()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()));

        self.file_system.push_path_to_object(path_to_upper_dir);
        self.file_system.push_path_to_render("..", true);

        for entry in walker.into_iter().filter_map(|e| e.ok()) {
            let depth = entry.depth();

            // TODO: Recursive depth limit set by left right keys
            if (depth > 1) || (depth == 0) || self.is_dotfile(&entry) {
                continue;
            }

            let prefix = "  ".repeat(depth - 1);

            self.file_system.push_path_to_render(
                &(prefix + &entry.file_name().to_string_lossy()),
                entry.path().is_dir());

            self.file_system.push_path_to_object(entry.into_path());
        }
    }

    fn change_dir(&mut self, target_path: &PathBuf) {
        // TODO: Handle invalid paths.
        self.clear_output();
        self.shell.set_cwd(
            std::fs::canonicalize(&target_path).unwrap_or(self.shell.cwd_as_path()));
        self.update_file_system();
        self.clear_input();
    }

    fn open_file(&mut self, target_path: &PathBuf) {
        
        let content = if target_path.exists() {
            fs::read_to_string(target_path).unwrap() 
        } else {
            return;
        };

        let editor = Editor::new("rust", content.as_str(), vesper()).unwrap();
       
        self.open_file = Some(target_path.clone());
        self.editor    = Some(editor);
    }

    fn handle_file_key_press (&mut self) 
    {
        let Some(file_index) = self.file_system.get_state().selected() else {
            return;
        };

        let target_path = self.file_system.get_path_to_object(file_index);
            
        if target_path.is_dir() {
            self.change_dir(&target_path);
        }
        return;
    }

    fn execute(&mut self) {
        let input = self.input.clone();
        let argv: Vec<&str> = input
                                .value()
                                .trim()
                                .split_whitespace()
                                .collect();

        if argv.len() == 0 {
            return;         
        }

        match argv[0] {
            "cd" => {
                if argv.len() > 1 {
                    let path_arg = PathBuf::from(argv[1]);

                    let target_path = if path_arg.is_absolute() {
                        path_arg
                    } else {
                        self.shell.get_cwd().join(path_arg)
                    };
                    if target_path.is_dir() {
                        self.change_dir(&target_path);
                    }
                    // TODO: Print invalid directory.
                }
            }

            _ => {
                self.send_cmd(argv);
            }
        }
    }

    fn clear_output(&mut self) {
        self.output.clear();
    }

    fn clear_input(&mut self) {
        self.input.reset();
    }

    fn traverse_dirs(&mut self, direction: Direction) {
        let k = match direction {
            Direction::UP => match self.file_system.get_state().selected() {
                Some(k) => {
                    if k <= 0 {
                        self.file_system.len() - 1
                    } else {
                        k - 1
                    }
                }
                None => 0,
            },
            Direction::DOWN => match self.file_system.get_state().selected() {
                Some(k) => {
                    if k >= self.file_system.len() - 1 {
                        0
                    } else {
                        k + 1
                    }
                }
                None => 0,
            } 
        };

        self.file_system.get_state().select(Some(k));
        
        if self.file_system.get_path_to_object(k).is_file()
        {
            self.open_file(&self.file_system.get_path_to_object(k));   
        }
    }

    fn handle_key_event(&mut self, key_event: KeyEvent, editor_area : &Rect) {
        match key_event.code {
            KeyCode::Esc   => {
                match self.focus {
                    Focus::SHELL |
                    Focus::FILES  => self.exit(),
                    Focus::EDITOR => {

                        let content = self.editor
                                        .as_ref()
                                        .unwrap()
                                        .get_content();


                        fs::write(self.open_file.as_ref().unwrap(), content);

                        // self.editor = None;
                        self.focus = Focus::SHELL;
                    },
                }
            },
            KeyCode::Down  => {
                match self.focus {
                  Focus::FILES  => self.traverse_dirs(Direction::DOWN),
                  Focus::SHELL  => {
                        // TODO: Handle shell history 
                  },
                  Focus::EDITOR =>  {
                        match &mut self.editor {
                            Some(editor) => { 
                                editor.input(key_event, editor_area);
                            },
                            None => {
                                /* Nothing to do */
                            },
                        }
                    },
                }
            },
            KeyCode::Up    => {
                
                match self.focus {
                    Focus::FILES  => self.traverse_dirs(Direction::UP),
                    Focus::SHELL  => {
                        // TODO: Handle shell history
                    },
                    Focus::EDITOR => {
                        match &mut self.editor {
                            Some(editor) => { 
                                editor.input(key_event, editor_area);
                            },
                            None => {
                                /* Nothing to do */
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
                        // TODO: Shell modifiers.
                    },
                    Focus::EDITOR => {
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
                    Focus::FILES  => self.focus = Focus::EDITOR, 
                    Focus::EDITOR => self.focus = Focus::SHELL,
                    Focus::SHELL  => self.focus = Focus::FILES,
                }
            },

            KeyCode::Enter => {
                
                match self.focus {
                    Focus::FILES  => {
                        self.handle_file_key_press(); 
                    },
                    Focus::SHELL  => {
                        self.execute();
                    },
                    Focus::EDITOR =>  {
                        match &mut self.editor {
                            Some(editor) => { 
                                editor.input(key_event, editor_area);
                            },
                            None => {
                                /* Nothing to do */
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
                    Focus::EDITOR =>  {
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

