/*****************************************************
 * Copyright 2026, Tide Project
 *****************************************************/

/**
 * This file implements the main application logic.
 */

/*****************************************************
 * Crates
 *****************************************************/

use crate::search::menu::SearchMenu;
use crate::popup_menu::PopupMenu;
use crate::input::Input;
use crate::shell::Shell;
use crate::file_system::FileTree;
use crate::search::SearchItemType;

use std::{ 
    cell::RefCell, 
    collections::HashMap, 
    fs, 
    io, 
    path::{
        Path, 
        PathBuf
    }, 
    rc::Rc, 
    time::{
        Duration,
        Instant
    }
};

use crossterm::event::{
        self, 
        Event, 
        KeyCode, 
        KeyEvent, 
        KeyEventKind, 
        MouseEvent, 
        MouseEventKind
    };

use crossbeam_channel::{
    Receiver, 
    unbounded
};

use ratatui::{
    DefaultTerminal, 
    Frame, 
    layout::{
        Constraint, 
        Layout, 
        Margin, 
        Position, 
        Rect, 
        Spacing
    }, style::{
        Color, 
        Modifier, 
        Style
    }, 
    symbols::merge::MergeStrategy, 
    text::Text,
    widgets::{
        Block, 
        Borders, 
        Clear, 
        Gauge, 
        List, 
        ListItem, 
        Paragraph
    }
};

use ratatui_code_editor::{
    editor::{ 
        Editor
    }, 
    theme::{
        vesper
    },
    actions::MoveDown
};

use ansi_to_tui::IntoText as _;

/*****************************************************
 * Types
 *****************************************************/

#[derive(PartialEq)]
enum EditorFocus {
    MAIN,
    MENU
}

#[derive(PartialEq)]
enum Focus {
    FILES,
    SHELL,
    EDITOR(EditorFocus),
    SEARCH
}

pub enum Direction {
    UP,
    DOWN,
}

pub enum MenuScreen {
    EDITOR(PopupMenu<String>), // TODO: Move into wrapper struct.
    SEARCH(SearchMenu)
}

pub struct App {
    input       : Input,
    file_system : FileTree,
    shell       : Rc<RefCell<Shell>>,
    exit        : bool,
    output      : Vec<String>,
    rx          : Receiver<Vec<u8>>, // TODO: Move into shell.rs 
    focus       : Focus,
    editor      : Option<Editor>,
    open_file   : Option<PathBuf>, // TODO: Move into editor wrapper struct. 
    menu_screen : Option<MenuScreen>, 
    last_scroll : Instant, // TODO: Move time markers into wrapper struct. 
    last_update : Instant, 
}

/*****************************************************
 * Local Functions 
 *****************************************************/

fn is_in_hitbox ((x,y) : (u16, u16),  rect : &Rect) -> bool
{
    (x > rect.x) && (x < (rect.x + rect.width )) &&
    (y > rect.y) && (y < (rect.y + rect.height))
}

/*****************************************************
 * Trait Implementations 
 *****************************************************/

impl From<KeyCode> for Direction {

    fn from(value: KeyCode) -> Self {
       match value {
         KeyCode::Up => Direction::UP,
         _ => Direction::DOWN
       }
    }
}

impl From<MouseEventKind> for Direction {

    fn from(value: MouseEventKind) -> Self {
       match value {
         MouseEventKind::ScrollUp => Direction::UP,
         _ => Direction::DOWN
       }
    }
}

/*****************************************************
 * Implementations
 *****************************************************/


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
            menu_screen : None,
            last_scroll : Instant::now(),
            last_update : Instant::now()
       }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
      
        let target_path = self.shell.borrow().cwd().to_path_buf();
        self.change_dir(&target_path);


        while !self.exit {
            
            while let Ok(bytes) = self.rx.try_recv() {
                let text = String::from_utf8_lossy(&bytes).to_string();
                let mut text = text.lines().map(String::from).collect();
                self.output.append(&mut text);
            }

            if let Some(MenuScreen::SEARCH(popup)) = &mut self.menu_screen {

                if let Some(fields) = { 
                    if let Some(search_rx) = popup.get_rx() {    
                        let mut fields = Vec::new();
                        while let Ok((score, item)) = search_rx.try_recv() {
                                      
                            if score > 0 
                            {
                                fields.push((score, item));
                            }
                        }

                        Some(fields)
                    } else {
                        None 
                    }
                } {
                    for (score, item) in fields {
                        popup.add_field(score, item);
                    }
                }; 
            }
            
            terminal.draw(|frame| self.draw(frame).expect("Failed to draw frame") )?;

            // TODO: wait until last command has completed! 
            if let Some(command) = self.input.pop_command() { 
                self.execute(command);  
            };

         }
        Ok(())
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    // TODO: insane amount of logic in this function. Error handling is non-existent.
    //       needs refactoring. Ideally rendering would be handed off to relevant 
    //       components, but API is tricky w/o context of UI elements.
    fn draw(&mut self, frame: &mut Frame) -> anyhow::Result<()> {

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
            
        let current_dir_path = self.file_system.get_current_dir_to_render();
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
 
        // Render text editor and shell output. TODO: cleanup.
       match &self.editor {
            Some (editor) => {
                frame.render_widget(editor, editor_area);
           
                let cursor = editor.get_visible_cursor(&editor_area);

                if let Some((x,y)) = cursor {
                    frame.set_cursor_position(Position::new(x,y));
                }
            }, 
            None => {
                
                let help = match &self.focus {
                                            Focus::FILES | Focus::SEARCH | Focus::SHELL => {
                                              vec![("Tab", "Change directory"),   
                                                   ("Enter", "Expand directory"),
                                                   ("Shift + Tab", "Cycle panes"), 
                                                   ("Esc", "Exit focus"), 
                                                   ("Up", "Scroll up"), 
                                                   ("Down", "Scroll down"),
                                                   ("Forward Slash", "Search current directory")]
                                            },
                                            _ => {
                                                vec![]
                                            },

                                          };
                let len = help.len().try_into().unwrap_or_default();

                if let Some(longest) = help.iter()
                                        .max_by_key(|(keybinding, _)| keybinding.len())
                {
                    let longest = longest.0.len();

                    let help = help
                            .iter()
                            .map(|(keybinding, help_text )| 
                                    keybinding.to_string() + 
                                    &" ".repeat(longest - keybinding.len()) + 
                                    " : " + 
                                    help_text
                                ).collect::<Vec<_>>().join("\n");


                    let text = Paragraph::new(help)
                            .style(Style::default().fg(Color::DarkGray));
                    let area =  editor_area.centered(
                                                Constraint::Length(text.clone().line_width() as u16),
                                                Constraint::Length(len));
                    frame.render_widget(text, area);
                }
            }
        }
        
        frame.render_widget(output, shell_output); 

        // Render any popup menus
        match &mut self.menu_screen {

            Some(MenuScreen::EDITOR(popup)) => {

                let popup_area_width = editor_area.width / 4;
                let popup_area_height = 10;

                let popup_area_x = editor_area.x + (editor_area.width  - popup_area_width ) / 2 ;
                let popup_area_y = editor_area.y + (editor_area.height - popup_area_height) / 2 ;

                let popup_area  = Rect::new(popup_area_x, popup_area_y, popup_area_width, popup_area_height);
                let popup_block = Block::default().borders(Borders::ALL);

                let popup_items = popup.get_list_items().to_vec();
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
                            .add_modifier(Modifier::BOLD),
                    ).highlight_symbol(">> ");

                frame.render_widget(Clear, popup_area);

                frame.render_stateful_widget(popup_list, popup_area, popup.get_state());
            },

            Some(MenuScreen::SEARCH(popup)) => {
                
                let frame_area = frame.area();

                let popup_area_width  = frame_area.width/2;
                let popup_area_height = (3 * frame_area.height)/4;

                let popup_area_x = frame_area.x + (frame_area.width  - popup_area_width ) / 2 ;
                let popup_area_y = frame_area.y + (frame_area.height - popup_area_height) / 2 ;

                let popup_area = Rect::new(popup_area_x, popup_area_y, popup_area_width, popup_area_height);
                
                let [title_area, search_area, input_area] = 
                    Layout::vertical([
                        Constraint::Min(3),
                        Constraint::Fill(24), 
                        Constraint::Min(1)
                    ]).split(popup_area)[..] 
                    else {
                        todo!() 
                    };   
               
                let title_block = Block::default().borders(Borders::TOP | Borders::RIGHT | Borders::LEFT);
                let search_block = Block::default().borders( Borders::RIGHT | Borders::LEFT);
                let input_block  = Block::default().borders(Borders::BOTTOM | Borders::RIGHT | Borders::LEFT);
 
                let popup_items = popup.get_list_items();
                let popup_items : Vec<ListItem> = popup_items.iter().enumerate()
                    .map(|(_, item)| {

                        let style = match item.item_type() {
                            SearchItemType::FILE => {
                                Style::default()
                                    .fg(Color::Green)
                                    .add_modifier(Modifier::BOLD)
                            },
                            SearchItemType::DIRECTORY => {
                                Style::default()
                                    .fg(Color::LightMagenta)
                                    .add_modifier(Modifier::BOLD)
                            },
                            SearchItemType::TEXT => {
                                Style::default()
                            },
                        };
                        
                        // TODO: Create meta data view, and group by file path 
                        // let text = k.metadata().unwrap_or_default().to_owned() + k.display();
                       
                        let (metadata, line_num) = item.metadata(); 
                    
                        let mut metadata = metadata.to_string_lossy().to_string();

                        let max_chars = 80;
                        
                        let n = metadata.len().saturating_sub(max_chars);
                        metadata.drain(0..n);

                        let line_num = if let Some(line_num) = line_num { format!(":{}", line_num) } else { "".to_owned() };
                       
                        let text = format!("{:}{:5}\n        ", metadata, line_num) + item.display(); 
                
                        ListItem::new(text).style(style)
                    })
                    .collect();

                let popup_list = List::new(popup_items)
                    .block(search_block)
                    .highlight_style(
                        Style::default()
                            .bg(Color::Yellow)
                            .fg(Color::Black)
                    .       add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol(">> ");

                frame.render_widget(Clear, popup_area);                
               
                let title = format!("{} {} items ", if popup.running() { "Searching" } else { "Searched" }, popup.get_n_items() );

                let title = Paragraph::new(title).block(title_block).right_aligned();                
                frame.render_widget(title, title_area);
               
                if popup.running() {
                    let refresh_time = 2.0;

										let elapsed_time = self.last_update.elapsed().as_secs_f64();

                    let ratio = if elapsed_time > refresh_time {
                            self.last_update = Instant::now(); 1.0 
                        } else { 
                            elapsed_time / refresh_time 
                    };
	                  
    
                    // TODO: Gauge rendering will be fixed when PR#2548 is merged and released
                    let percent = (100.0 * ratio) as u16;

                    let gauge = Gauge::default().block(input_block).percent(percent).label(" Searching");
										frame.render_widget(gauge, input_area);
                } else {
                    let input = Paragraph::new(" > ".to_owned() + popup.get_input_to_render()).block(input_block);                   
                
                    frame.render_widget(input, input_area);
                }                

                frame.render_stateful_widget(popup_list, search_area, popup.get_state());
            },

            _ => { 
                /* Nothing to render. */ 
            }
        }
  
        // Create a rect which maps to the entire shell area for handling mouse events.
        let shell_area = Rect::new(shell_output.x, shell_output.y, shell_output.width, shell_output.height + shell_input.height);
        
        // Handle events after drawing the layout so we can use the areas drawn as part of the app.
        self.handle_events(&editor_area, &shell_area, &file_area)?;

        Ok(())
    }

    fn handle_events(&mut self, editor_area : &Rect, shell_area : &Rect, file_area : &Rect) -> io::Result<()> {
        if event::poll(Duration::from_millis(10))? {
            match event::read()? {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    self.handle_key_event(key_event, editor_area)
                }
                Event::Mouse(mouse_event) => { 
                    self.handle_mouse_event(mouse_event, editor_area, shell_area, file_area); 
                }
                _ => { /* Nothing to do. */ }
            }
        }
        Ok(())
    }

    fn change_dir(&mut self, target_path: &Path) {
              
        let target_path = std::fs::canonicalize(target_path).unwrap_or(self.shell.borrow_mut().cwd().to_path_buf());

        self.shell.borrow_mut().set_cwd(target_path.clone());

        self.file_system.change_dir(target_path);
    }

    fn open_file(&mut self, target_path: &PathBuf) {

        let extension_to_language_map = HashMap::from([("", ""), ("c", "c"), ("h", "cpp"), ("cpp", "cpp"), ("rs", "rust"), ("md", "markdown")]);
                
        let content = if target_path.exists() {
            match fs::read_to_string(target_path) {
                Ok(ok)  => ok,
                Err(_err) => return
            }
        } else {
            return;
        };
        let extension = target_path.extension().map(|path| path.to_str().unwrap_or_default()).unwrap_or_default();
        let lang = {
            if let Some(lang) = extension_to_language_map.get(extension) {
                lang.to_string()
            } else {
                "shell".to_string()
            }
        };

        let editor = Editor::new(&lang, content.as_str(), vesper()).unwrap();
       
        self.open_file = Some(target_path.clone());
        self.editor    = Some(editor);
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
                        self.shell.borrow_mut().cwd().to_path_buf().join(path_arg)
                    };
                    if target_path.is_dir() {
                        self.change_dir(&target_path);
                    }
                }
            },
            "export" => { 

                if argv.len() > 1 {
                   
                    let [variable, value] =  argv[1].split("=").collect::<Vec<&str>>()[..] else {
                        todo!()
                    };
   
                    self.shell.borrow_mut().set_env(variable, value);
                }

            }

            _ => {
                self.shell.borrow_mut().send_cmd(argv);
            }
        }

    }
    
    fn handle_mouse_event(&mut self, mouse_event : MouseEvent, editor_area : &Rect, shell_area : &Rect, file_area : &Rect) {
        
        // First handle mouse event clicks. Left mouse sets focus. 
        if let MouseEventKind::Down(mouse_button) = mouse_event.kind { 
            
            const OFFSET_TO_FIRST_ENTRY : u16 = 1;

            let x = mouse_event.column;
            let y = mouse_event.row;

            if mouse_button.is_left() {
                if is_in_hitbox((x, y), editor_area) {
                    if let Some(_editor) = &self.editor {
                        self.focus = Focus::EDITOR(EditorFocus::MAIN);
                    };
                } else if is_in_hitbox((x, y), shell_area) {
                    self.focus = Focus::SHELL;
                } else if is_in_hitbox((x, y), file_area) {
                    self.focus = Focus::FILES;
 
                    let k = (y - OFFSET_TO_FIRST_ENTRY) as usize;

                    if let Some(file) = self.file_system.select_entry(k) {
                        self.open_file(&file);
                    }
                }   
            }
            if mouse_button.is_right() {
            
                if is_in_hitbox((x,y), file_area) {
 
                    let k = (y - OFFSET_TO_FIRST_ENTRY) as usize;

                    let Some(dir) = self.file_system.get_dir_at_index(k) else {
                        return;
                    };

                    let dir = dir.to_path_buf();

                    self.change_dir(&dir);
                }

            }
        };  

        match &self.focus {
            Focus::SHELL | Focus::SEARCH => {
                
                
            },
            Focus::FILES => {

                match mouse_event.kind {
                    MouseEventKind::ScrollUp | 
                    MouseEventKind::ScrollDown if self.last_scroll.elapsed() > Duration::from_millis(250) => {
                        if let Some(file) = self.file_system.traverse_dirs(Direction::from(mouse_event.kind)) {
                            self.open_file(&file);
                        };

                        self.last_scroll = Instant::now();
                    },
                    _ => {
                        /* Nothing to do. */
                    },

                }

            },
            Focus::EDITOR(editor_focus) => {
                match editor_focus {
                    EditorFocus::MAIN => {
                        if let Some(editor) = &mut self.editor {
                            let _ = editor.mouse(mouse_event, editor_area);
                        };
                    },
                    EditorFocus::MENU => {

                    },
                }
            }

        }
    }
    
    fn handle_key_event(&mut self, key_event: KeyEvent, editor_area : &Rect) {
        match key_event.code {
            KeyCode::Esc   => {
                match &self.focus {
                    Focus::SHELL |
                    Focus::FILES  => self.exit(),
                    Focus::SEARCH => {
                        
                        // Cleanup after leaving search
                        if let Some(MenuScreen::SEARCH(popup)) = &mut self.menu_screen {
                            popup.cleanup();
                        };

                        self.menu_screen = None;
                        self.focus       = Focus::FILES;
                    },  
                    Focus::EDITOR(editor_focus) => {

                        match editor_focus {

                            EditorFocus::MAIN => {
                               self.menu_screen = Some(MenuScreen::EDITOR(PopupMenu::default().add_field("Save?".to_owned()).add_field("Exit?".to_owned())));
                                self.focus       = Focus::EDITOR(EditorFocus::MENU);
                            },

                            EditorFocus::MENU => {   

                                self.menu_screen = None;
                                self.focus       = Focus::EDITOR(EditorFocus::MAIN); 
                            }
                        }
                    },
                }
            },
            KeyCode::Down | KeyCode::Up  => {
                match &self.focus {

                  Focus::SEARCH => {
                    if let Some(MenuScreen::SEARCH(popup)) = &mut self.menu_screen {
                        popup.traverse_items(Direction::from(key_event.code));
                    };
                  },
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
                                    let _ = editor.input(key_event, editor_area);
                                };
                            },
                            EditorFocus::MENU => {
                                if let Some(MenuScreen::EDITOR(popup)) = &mut self.menu_screen {
                                    popup.traverse_items(Direction::from(key_event.code));
                                };
                            },
                        }
                    },
                }
            }, 
            
            KeyCode::Modifier(_modifiier) => {

                match self.focus {
                    Focus::FILES | Focus::SEARCH => {
                        // TODO: File modifiiers.
                    },
                    Focus::SHELL => {
                        self.input.handle_event(&Event::Key(key_event));
                    },
                    Focus::EDITOR(_) => {
                        match &mut self.editor {
                            Some(editor) => {
                                let _ =editor.input(key_event, editor_area);
                            },
                            None => {
                                /* Nothing to do */
                            }          
                        }
                    }
                }
            },

            KeyCode::Tab if self.focus == Focus::FILES => {
            
               let Some(selected) = self.file_system.get_selected_dir() else {
                   return;
               };
               let selected = selected.to_path_buf();

               self.change_dir(&selected); 
            }

            KeyCode::Tab if self.focus == Focus::SEARCH => {
               
                               
                let mut line_num    = 0;
                let new_focus = if let Some(MenuScreen::SEARCH(popup)) = &mut self.menu_screen {
                   
                    let Some(item) = popup.get_selected_item() else {
                        return
                    };
                    
                    // TODO: Path returns string, should return &Path. This is pretty horrible logic. 
                    let path = PathBuf::from(item.metadata().0);
 
                    if path.is_dir() { 
                        
                        self.change_dir(&path);

                        Focus::FILES                        

                    } else {

                        line_num = item.metadata().1.unwrap_or(1);

                        self.open_file(&path);
  
                        Focus::EDITOR(EditorFocus::MAIN)
                    }
                    
                   
                } else { 
                    Focus::SEARCH
                };   
                
                if self.focus != new_focus {

                    self.focus = new_focus;
                    self.menu_screen = None;

                    if let Some(editor) = &mut self.editor 
                    {                            
                        for _ in 0..line_num
                        {
                            editor.apply(MoveDown{shift : false }); 
                        }
                        
                        // Force editor refresh.
                        editor.focus(&editor_area);
                    }
                }
            }

            KeyCode::BackTab => {
                
                match self.focus {
                    Focus::SEARCH => {

                    },
                    Focus::FILES     => {
                        if let Some(_editor) = &self.editor {
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
                    Focus::SEARCH => {

                        let cwd = self.shell.borrow()
                                            .cwd()
                                            .to_path_buf();

                        if let Some(MenuScreen::SEARCH(popup)) = &mut self.menu_screen {
                            popup.search(&cwd);
                        } 
                    },
                    Focus::FILES  => {
                        if ! self.file_system.toggle_dir(false) {
                            self.focus = Focus::EDITOR(EditorFocus::MAIN);
                        }
                    },
                    Focus::SHELL  => {
                        self.input.handle_event(&Event::Key(key_event));
                    },
                    Focus::EDITOR(editor_focus) => {
                        match editor_focus {
                            EditorFocus::MAIN => {
                                if let Some(editor) = &mut self.editor {
                                    let _ = editor.input(key_event, editor_area);
                                };
                            },
                            EditorFocus::MENU => { 
                                let mut close_menu = false;
                                if let Some(MenuScreen::EDITOR(popup_menu)) = &mut self.menu_screen {

                                    if popup_menu.selected("Save?".to_owned()) {

                                        let content = self.editor.as_ref()
                                                                 .unwrap()
                                                                 .get_content();

                                        let _ = fs::write(self.open_file.as_ref().unwrap(), content);
                                    
                                    }

                                    if popup_menu.selected("Exit?".to_owned()) {
                                        self.editor = None;

                                        self.focus  = Focus::FILES;

                                        close_menu  = true;     
                                    }
                                };

                                if close_menu {
                                    self.menu_screen = None;
                                }
                            },
                        }
                    },
                }
            },

            KeyCode::Char('/') => {   

                // TODO: Apply some context about what we should search based on previous focus
                self.menu_screen = Some(MenuScreen::SEARCH(SearchMenu::default()));
                self.focus       = Focus::SEARCH; 
                
            }

            _ => {
                match self.focus {
                    Focus::SEARCH => {
                        if let Some(MenuScreen::SEARCH(popup)) = &mut self.menu_screen {
                            popup.handle_event(&Event::Key(key_event));
                        };
                    },
                    Focus::FILES => {
                        // TODO: Handle other keys
                    },  
                    Focus::SHELL => {
                        self.input.handle_event(&Event::Key(key_event));
                    },
                    Focus::EDITOR(_) =>  {
                        match &mut self.editor {
                            Some(editor) => { 
                                let _ = editor.input(key_event,editor_area);
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

