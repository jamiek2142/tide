/*****************************************************
 * Copyright 2026, Tide Project
 *****************************************************/

use crate::file_system::{FileTree, FileType};
use crate::input::Input;
use crate::popup_menu::PopupMenu;
use crate::search::SearchItemType;
/**
 * This file implements the main application logic.
 */
/*****************************************************
 * Crates
 *****************************************************/
use crate::search::menu::SearchMenu;
use crate::shell::Shell;

use std::{
    cell::RefCell, collections::HashMap, fs, io, path::{Path, PathBuf}, rc::Rc, thread, time::{Duration, Instant}
};

use crossbeam_channel::{Receiver, unbounded};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind};

use pathdiff::diff_paths;

use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout, Margin, Position, Rect, Spacing},
    style::{Color, Modifier, Style},
    symbols::merge::MergeStrategy,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Tabs},
};

use ratatui_code_editor::{actions::MoveDown, editor::Editor, theme::vesper};

use ansi_to_tui::IntoText as _;

/*****************************************************
 * Types
 *****************************************************/

#[derive(PartialEq)]
enum EditorFocus {
    MAIN,
    MENU,
}

#[derive(PartialEq)]
enum Focus {
    FILES,
    SHELL,
    EDITOR(EditorFocus),
    SEARCH,
}

pub enum Direction {
    UP,
    DOWN,
}

pub enum MenuScreenType {
    EDITOR,
    SEARCH
}

pub enum MenuScreen {
    EDITOR(PopupMenu<String>), // TODO: Move into wrapper struct.
    SEARCH(SearchMenu),
}

#[derive(PartialEq)]
pub enum DragKind {
    VERTICAL,
    HORIZONTAL,
}

pub struct Split {
    horizontal: u16,
    vertical: u16,
}

pub struct EditorPane {
    pane : Editor,
    path : PathBuf,
    hash : u64
}

pub struct App {
    input: Input,
    file_system: FileTree,
    shell: Rc<RefCell<Shell>>,
    exit: bool,
    output: Vec<String>,
    output_pos: u16,
    focus: Focus,
    preview_pane: Option<EditorPane>,
    editor_panes: Vec<EditorPane>,
    selected_editor: Option<usize>, 
    menu_screen: Option<MenuScreen>,
    event_rx : Receiver<Event>,
    // last_scroll: Instant, //< TODO: Move time markers into wrapper struct.
    last_update: Instant,
    split: Split,
    last_drag: Option<DragKind>,
}

/*****************************************************
 * Local Functions
 *****************************************************/

fn is_in_hitbox((x, y): (u16, u16), rect: &Rect) -> bool {
    (x > rect.x) && (x < (rect.x + rect.width)) && (y > rect.y) && (y < (rect.y + rect.height))
}

fn get_path_to_render(path: &Path, avaiable_space: u16) -> String {
    let mut required_space = 0;
    let mut num_elems = 0;

    for elem in path.iter().rev() {
        let num_chars: u16 = elem.to_str().unwrap().chars().count().try_into().unwrap();

        required_space = required_space + num_chars;

        if required_space > avaiable_space {
            break;
        }

        num_elems = num_elems + 1;
    }

    let num_components = path.components().count();

    if num_elems < num_components {
        let dir_path_to_render: PathBuf =
            path.components().skip(num_components - num_elems).collect();

        "../".to_string() + &dir_path_to_render.to_string_lossy()
    } else {
        path.to_string_lossy().to_string()
    }
}

/*****************************************************
 * Trait Implementations
 *****************************************************/

impl From<KeyCode> for Direction {
    fn from(value: KeyCode) -> Self {
        match value {
            KeyCode::Up => Direction::UP,
            _ => Direction::DOWN,
        }
    }
}

impl From<MouseEventKind> for Direction {
    fn from(value: MouseEventKind) -> Self {
        match value {
            MouseEventKind::ScrollUp => Direction::UP,
            _ => Direction::DOWN,
        }
    }
}

impl Default for Split {
    fn default() -> Self {
        Self {
            horizontal: 20,
            vertical: 80,
        }
    }
}

/*****************************************************
 * Implementations
 *****************************************************/

impl Split {
    pub fn get_horizontal_hitbox(&self, frame: &Rect) -> Rect {
        const TOLERANCE: u16 = 2;

        let center = (self.horizontal * frame.width) / 100;

        let x = center.saturating_sub(TOLERANCE);
        let width = center.saturating_add(TOLERANCE) - x;
        let y = frame.y;

        let height = frame.height;

        Rect {
            x,
            y,
            width,
            height,
        }
    }

    pub fn get_vertical_split_hitbox(&self, frame: &Rect) -> Rect {
        const TOLERANCE: u16 = 3;

        let center = (self.vertical * frame.height) / 100;

        let y = center.saturating_sub(TOLERANCE);
        let height = center.saturating_add(TOLERANCE) - y;

        let x = frame
            .x
            .saturating_add((self.horizontal * frame.width) / 100);
        let width = frame.width - x;

        Rect {
            x,
            y,
            width,
            height,
        }
    }

    pub fn set_horizontal_percentage(&mut self, point: (u16, u16), frame: &Rect) {
        self.horizontal = point.0 * 100 / frame.width;
    }

    pub fn set_vertical_percentage(&mut self, point: (u16, u16), frame: &Rect) {
        self.vertical = point.1 * 100 / frame.height;
    }

    pub fn get_horizontal_split_percentage(&self) -> (u16, u16) {
        const MAX_PERCENT: u16 = 100;
        (self.horizontal, MAX_PERCENT.saturating_sub(self.horizontal))
    }

    pub fn get_vertical_split_percentage(&self) -> (u16, u16) {
        const MAX_PERCENT: u16 = 100;
        (self.vertical, MAX_PERCENT.saturating_sub(self.vertical))
    }
}

impl App {
    pub fn new() -> Self {
        let shell = Rc::new(RefCell::new(Shell::new()));

        let (tx, rx) = unbounded();

        /* Start up the background thread which will handle events */
        thread::spawn(move || { 
            loop {
                let event = event::read().unwrap();
                let _ = tx.send(event);
            }
        });

        Self {
            input: Input::new(),
            file_system: FileTree::new(Rc::clone(&shell)),
            shell: shell,
            exit: bool::default(),
            output: Vec::new(),
            output_pos: 0,
            focus: Focus::FILES,
            preview_pane: None,
            editor_panes: Vec::new(),
            selected_editor: None,
            menu_screen: None,
            event_rx: rx,
            //last_scroll: Instant::now(),
            last_update: Instant::now(),
            split: Split::default(),
            last_drag: None,
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        let target_path = self.shell.borrow().cwd().to_path_buf();
        self.change_dir(&target_path);

        while !self.exit {
            while let Ok(bytes) = self.shell.borrow().rx().try_recv() {
                let text = String::from_utf8_lossy(&bytes).to_string();
                let mut text = text.lines().map(String::from).collect();
                self.output.append(&mut text);
            }

            if let Some(MenuScreen::SEARCH(popup)) = &mut self.menu_screen {
                if let Some(fields) = {
                    if let Some(search_rx) = popup.get_rx() {
                        let mut fields = Vec::new();
                        while let Ok((score, item)) = search_rx.try_recv() {
                            if score > 0 {
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

            terminal.draw(|frame| self.draw(frame).expect("Failed to draw frame"))?;

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
    //       needs refactoring. Ideally/ rendering would be handed off to relevant
    //       components, but API is tricky w/o context of UI elements.
    fn draw(&mut self, frame: &mut Frame) -> anyhow::Result<()> {
        let (left, right) = self.split.get_horizontal_split_percentage();
        let (top, bottom) = self.split.get_vertical_split_percentage();

        let [file_area, main_area] =
            Layout::horizontal([Constraint::Percentage(left), Constraint::Percentage(right)])
                .spacing(Spacing::Overlap(1))
                .split(frame.area())[..]
        else {
            todo!()
        };
        let [editor_area, shell_output, shell_input] = Layout::vertical([
            Constraint::Percentage(top),
            Constraint::Percentage(bottom),
            Constraint::Min(2),
        ])
        .split(main_area)[..] else {
            todo!()
        };
 
        let text = " > ".to_string() + self.input.get_input_to_render();

        let input_block = {
            let block = Block::new()
                .borders(Borders::LEFT)
                .merge_borders(MergeStrategy::Exact);

            match self.focus {
                Focus::SHELL => block.border_style(Style::new().light_green()),
                _ => block,
            }
        };

        let input = Paragraph::new(text)
            .style(Style::default())
            .block(input_block);

        let start_pos = (self.output.len() as u16).saturating_sub(shell_output.height + self.output_pos) as usize;

        let last_lines = &self.output[start_pos..];

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
                Focus::SHELL => block.border_style(Style::new().light_green()),
                _ => block,
            }
        };

        let output = Paragraph::new(text)
            .style(Style::default())
            .block(output_block);

        let items: Vec<ListItem> = self
            .file_system
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
                Focus::FILES => block.border_style(Style::new().light_green()),
                _ => block,
            }
        };

        let current_dir_path = self.file_system.get_current_dir_to_render();

        let dir_path_to_render = get_path_to_render(&current_dir_path, 15);

        let list = List::new(items)
            .block(
                files_block.title(dir_path_to_render).title_style(
                    Style::default()
                        .fg(Color::LightMagenta)
                        .add_modifier(Modifier::BOLD),
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
            let cursor_offset_x = 4 + shell_input.x + self.input.get_cursor_position();
            let cursor_offset_y = shell_input.y;

            frame.set_cursor_position((cursor_offset_x, cursor_offset_y));
        }

        // Render text editor and shell output. TODO: cleanup.
        let editor_area = match self.selected_editor {
            Some(index)  => {
 
			        let tabs  = Tabs::new(self.editor_panes.iter().map(|editor| {

      		      let content = editor.pane.get_content();
                let hash = crc64::crc64(0, content.as_bytes());

                editor.path
          	      .file_name()
            	    .to_owned()
              	  .unwrap_or_default()
                	.to_string_lossy()
                	.to_string() + if hash != editor.hash { "*" } else { "" } 
           	 		}).collect::<Vec<String>>())
									.select(self.selected_editor.unwrap_or_default())
									.style(Color::White)
        					.highlight_style(Style::default().magenta().on_black().bold());

                let editor = & self.editor_panes[index];

                let [tabs_area, editor_area] = Layout::vertical([
                        Constraint::Min(2),
                        Constraint::Percentage(100)
                    ]).split(editor_area)[..] 
                else { 
                    todo!() 
                };

                let tabs_area = tabs_area.inner(Margin::new(1, 0));
                
                frame.render_widget(Block::new().borders(Borders::BOTTOM), tabs_area); 

                let editor_area = editor_area.inner(Margin::new(1, 0));
                
                frame.render_widget(&editor.pane, editor_area);

                if let Some(preview_editor) = &mut self.preview_pane {

                    let preview_area = editor_area.inner(Margin::new(10, 2));

                    frame.render_widget(Clear, preview_area);
                    frame.render_widget(Block::bordered(), preview_area);
                    let preview_area = preview_area.inner(Margin::new(1,1));

                    frame.render_widget(&preview_editor.pane,preview_area); 
                };
                
                frame.render_widget(tabs, tabs_area);
                
                let cursor = editor.pane.get_visible_cursor(&editor_area);

                if let Some((x, y)) = cursor {
                    frame.set_cursor_position(Position::new(x, y));
                }

                editor_area
            }
            None => {
                
                if let Some(preview_editor) = &mut self.preview_pane {

                    let editor_area = editor_area.inner(Margin::new(1, 0));
                    
                    let preview_area = editor_area.inner(Margin::new(10, 2));

                    frame.render_widget(Clear, preview_area);
                    frame.render_widget(Block::bordered(), preview_area);
                    let preview_area = preview_area.inner(Margin::new(1,1));


                    frame.render_widget(&preview_editor.pane,preview_area);
                    
                    editor_area
                } else {
               
                let help = match &self.focus {
                    Focus::FILES | Focus::SEARCH | Focus::SHELL => {
                        vec![
                            ("Tab", "Change directory | Load File"),
                            ("Enter", "Expand directory | Open File"),
                            ("Shift + Tab", "Cycle panes"),
                            ("Esc", "Exit focus"),
                            ("Up", "Scroll up"),
                            ("Down", "Scroll down"),
                            ("?", "Open search menu")
                        ]
                    }
                    _ => {
                        vec![]
                    }
                };
                let len = help.len().try_into().unwrap_or_default();

                if let Some(longest) = help.iter().max_by_key(|(keybinding, _)| keybinding.len()) {
                    let longest = longest.0.len();

                    let help = help
                        .iter()
                        .map(|(keybinding, help_text)| {
                            keybinding.to_string()
                                + &" ".repeat(longest - keybinding.len())
                                + " : "
                                + help_text
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    let text = Paragraph::new(help).style(Style::default().fg(Color::DarkGray));
                    let area = editor_area.centered(
                        Constraint::Length(text.clone().line_width() as u16),
                        Constraint::Length(len),
                    );
                    frame.render_widget(text, area);

                }
                    Rect::default()
                }
            }
        };

        frame.render_widget(output, shell_output);

        // Get a copy of the working directory for later. 
        let working_dir = self.shell.borrow().cwd().to_path_buf();

        // Render any popup menus
        let menu_screen = match &mut self.menu_screen {
            Some(MenuScreen::EDITOR(popup)) => {
                let popup_area_width = {
                    let mut max_len = 0;

                    for element in popup.get_list_items() {
                        if element.chars().count() > max_len {
                            max_len = element.chars().count();
                        }
                    }

                    max_len as u16
                } + 8;

                let popup_area_height = { popup.get_list_items().iter().count() as u16 } + 2;

                let popup_area_x = editor_area.x + (editor_area.width - popup_area_width) / 2;
                let popup_area_y = editor_area.y + (editor_area.height - popup_area_height) / 2;

                let popup_area = Rect::new(
                    popup_area_x,
                    popup_area_y,
                    popup_area_width,
                    popup_area_height,
                );
                let popup_block = Block::default().borders(Borders::ALL);

                let popup_items = popup.get_list_items().to_vec();
                let popup_items: Vec<ListItem> = popup_items
                    .iter()
                    .map(|k| ListItem::new(k.as_str()))
                    .collect();

                let popup_list = List::new(popup_items)
                    .block(popup_block)
                    .highlight_style(
                        Style::default()
                            .bg(Color::Yellow)
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol(">> ");

                frame.render_widget(Clear, popup_area);

                frame.render_stateful_widget(popup_list, popup_area, popup.get_state());

                Some((MenuScreenType::EDITOR, popup_area))
            }

            Some(MenuScreen::SEARCH(popup)) => {

                /* Get bounds for the search layout. */
                let frame_area = frame.area();

                let popup_area_width = frame_area.width / 2;
                let popup_area_height = (3 * frame_area.height) / 4;

                let popup_area_x = frame_area.x + (frame_area.width - popup_area_width) / 2;
                let popup_area_y = frame_area.y + (frame_area.height - popup_area_height) / 2;

                let popup_area = Rect::new(
                    popup_area_x,
                    popup_area_y,
                    popup_area_width,
                    popup_area_height,
                );
                
                /* Create 3 split areas and corresponding blocks, one for the search bar, one for the results, and a title bar. */
                let [title_area, search_area, input_area] = Layout::vertical([
                    Constraint::Min(3),
                    Constraint::Fill(24),
                    Constraint::Min(2),
                ])
                .split(popup_area)[..] else {
                    todo!()
                };

                let title_block = {
                    let block = Block::default().borders(Borders::TOP | Borders::RIGHT | Borders::LEFT);
                    
                    if let Focus::SEARCH = self.focus {
                        block.border_style(Style::new().light_green())
                    } else { 
                        block 
                    }
                };
                let search_block = {
                    let block = Block::default().borders(Borders::RIGHT | Borders::LEFT);

                    if let Focus::SEARCH = self.focus {
                        block.border_style(Style::new().light_green())
                    } else { 
                        block 
                    }
                };
                let input_block = {
                    let block = Block::default().borders(Borders::BOTTOM | Borders::RIGHT | Borders::LEFT);

                    if let Focus::SEARCH = self.focus {
                        block.border_style(Style::new().light_green())
                    } else { 
                        block 
                    }
                };
                
                /* Render the search list of items. */
                let popup_items = popup.get_list_items();
                let popup_items: Vec<ListItem> = popup_items
                    .iter()
                    .enumerate()
                    .map(|(_, item)| {
                        let style = match item.item_type() {
                            SearchItemType::FILE => Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                            SearchItemType::DIRECTORY => Style::default()
                                .fg(Color::LightMagenta)
                                .add_modifier(Modifier::BOLD),
                            SearchItemType::TEXT => Style::default(),
                        };

                        let (metadata, line_num) = item.metadata();

                        let path = diff_paths(metadata, &working_dir)
                            .expect("Failed to get relative path");

                        let metadata = get_path_to_render(&path, search_area.width);

                        let meta_text = Span::styled(
                            metadata,
                            Style::default()
                                .fg(Color::LightMagenta)
                                .add_modifier(Modifier::BOLD),
                        );

                        let line_text = Span::raw(if let Some(line_num) = line_num {
                            format!(":{}\n", line_num)
                        } else {
                            "\n".to_owned()
                        });

                        let meta_text = Line::from(vec![meta_text, line_text]);

                        let result_text = Span::raw(item.display());

                        let text = Text::from(vec![meta_text.into(), result_text.into()]);

                        ListItem::new(text).style(style)
                    })
                    .collect();
                
                 let popup_list = List::new(popup_items)
                    .block(search_block)
                    .highlight_style(
                        Style::default()
                            .bg(Color::Yellow)
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol(">> ");

                frame.render_widget(Clear, popup_area);

                /* Create search popup title. */
                let title = format!(
                    "{} {} items ",
                    if popup.running() {
                        "Searching"
                    } else {
                        "Searched"
                    },
                    popup.get_n_items()
                );

                let title = Paragraph::new(title).block(title_block).right_aligned();
                frame.render_widget(title, title_area);
                
                /* Create the search progress bar. */ 
                if popup.running() {
                    let refresh_time = 2.0;

                    let elapsed_time = self.last_update.elapsed().as_secs_f64();

                    let ratio = if elapsed_time > refresh_time {
                        self.last_update = Instant::now();
                        1.0
                    } else {
                        elapsed_time / refresh_time
                    };

                    // TODO: Gauge rendering will be fixed when PR#2548 is merged and released
                    let percent = (100.0 * ratio) as u16;

                    let gauge = Gauge::default()
                        .block(input_block)
                        .percent(percent)
                        .label(" Searching");
                    frame.render_widget(gauge, input_area);
                } else {
                    let input = Paragraph::new(" > ".to_owned() + popup.get_input_to_render())
                        .block(input_block);

                    frame.render_widget(input, input_area);
                }

                frame.render_stateful_widget(popup_list, search_area, popup.get_state());

                Some((MenuScreenType::SEARCH, search_area))
            }

            _ => { None }
        };

        // Create a rect which maps to the entire shell area for handling mouse events.
        let shell_area = Rect::new(
            shell_output.x,
            shell_output.y,
            shell_output.width,
            shell_output.height + shell_input.height,
        );

        // Handle events after drawing the layout so we can use the areas drawn as part of the app.
        self.handle_events(&frame.area(), &editor_area, &shell_area, &file_area, menu_screen.as_ref())?;

        Ok(())
    }

    fn handle_events(
        &mut self,
        frame_area: &Rect,
        editor_area: &Rect,
        shell_area: &Rect,
        file_area: &Rect,
        popup_area : Option<&(MenuScreenType, Rect)>
    ) -> io::Result<()> {
        
        /* 
         * Pull event - don't block forever as we need to keep rendering dynamic items. 
         * Minimum frame rate is ~20 fps. TODO: Let this be user configurable.  
         */ 
        if let Ok(event) = self.event_rx.recv_timeout(Duration::from_millis(50)) {
           
            // Collect any events which have occured during the timeout to maintain responsiveness. 
            let mut events = vec![event];
            events.extend(self.event_rx.try_iter());

            for event in events
            {
                match event {
                    Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                        self.handle_key_event(key_event, editor_area);
                    }
                    Event::Mouse(mouse_event) => {
                        self.handle_mouse_event(
                            mouse_event,
                            frame_area,
                            editor_area,
                            shell_area,
                            file_area,
                            popup_area
                        );
                    }
                    _ => { /* Nothing to do. */ }
                }
            }
        }
        Ok(())
    }

    fn change_dir(&mut self, target_path: &Path) {
        let target_path = std::fs::canonicalize(target_path)
            .unwrap_or(self.shell.borrow_mut().cwd().to_path_buf());

        self.shell.borrow_mut().set_cwd(target_path.clone());

        self.file_system.change_dir(target_path);
    }

    fn load_file(&mut self, target_path: &Path) -> Option<(Editor, u64)>  {
        let extension_to_language_map = HashMap::from([
            ("", ""),
            ("c", "c"),
            ("h", "cpp"),
            ("cpp", "cpp"),
            ("rs", "rust"),
            ("md", "markdown"),
        ]);

        let content = if target_path.exists() {
            match fs::read_to_string(target_path) {
                Ok(ok) => ok,
                Err(_err) => return None,
            }
        } else {
            return None;
        };
        let extension = target_path
            .extension()
            .map(|path| path.to_str().unwrap_or_default())
            .unwrap_or_default();
        let lang = {
            if let Some(lang) = extension_to_language_map.get(extension) {
                lang.to_string()
            } else {
                "shell".to_string()
            }
        };
            
        
        Some((Editor::new(&lang, content.as_str(), vesper()).expect("Failed to open editor"), crc64::crc64(0, content.as_bytes())))
    }

    fn preview_file(&mut self, target_path: &Path) {
        
        let Some((editor, hash)) = self.load_file(target_path) else {
            return;
        };
 
        self.preview_pane = Some(EditorPane { pane : editor, path : target_path.to_path_buf(), hash : hash});
    }

    fn open_file(&mut self, target_path: &Path) {
        
        for (k, editor_pane) in self.editor_panes.iter().enumerate() {    
            if editor_pane.path == *target_path {
               self.preview_pane = None;
               self.selected_editor = Some(k);
							 return; 
            }
        }
        
        let Some((editor, hash)) = self.load_file(target_path) else {
            return;
        }; 
 
        self.preview_pane = None;
        self.editor_panes.push( EditorPane { pane: editor, path: target_path.to_path_buf(), hash: hash });
        self.selected_editor = Some(self.editor_panes.len() - 1);  
        
    }

    fn execute(&mut self, command: String) {
        let argv: Vec<&str> = command.trim().split_whitespace().collect();

        if argv.len() == 0 {
            return;
        }

        match argv[0] {
            "set-vertical" => {
                if argv.len() > 1 {
                    self.split.vertical = (|x: &str| {
                        let x = x.parse::<u16>().unwrap_or(50);

                        if x > 100 { 100 } else { x }
                    })(argv[1]);
                }
            }

            "set-horizontal" => {
                if argv.len() > 1 {
                    self.split.horizontal = (|x: &str| {
                        let x = x.parse::<u16>().unwrap_or(50);
                        if x > 100 { 100 } else { x }
                    })(argv[1]);
                }
            }

            "clear" => {
                self.output.clear();
            }
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
            }
            "export" => {
                if argv.len() > 1 {
                    let [variable, value] = argv[1].split("=").collect::<Vec<&str>>()[..] else {
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

    fn handle_mouse_event(
        &mut self,
        mouse_event: MouseEvent,
        frame_area: &Rect,
        editor_area: &Rect,
        shell_area: &Rect,
        file_area: &Rect,
        popup_area : Option<&(MenuScreenType, Rect)>
    ) {
        let x = mouse_event.column;
        let y = mouse_event.row;

        if is_in_hitbox((x,y), editor_area) {
            if let Some(index) = self.selected_editor {
                let _ = self.editor_panes[index].pane.mouse(mouse_event, editor_area);
            };
        }

        match mouse_event.kind {
            MouseEventKind::Drag(_mouse_button) => {
                match self.last_drag {
                    Some(DragKind::VERTICAL) => {
                        self.split.set_vertical_percentage((x, y), frame_area);
                    }
                    Some(DragKind::HORIZONTAL) => {
                        self.split.set_horizontal_percentage((x, y), frame_area);
                    }
                    None => { /* Nothing to do. */ }
                }
            }

            MouseEventKind::Up(_mouse_button) => {
                // Clear previous drag.
                if let Some(_) = self.last_drag {
                    self.last_drag = None;
                };
            }

            // First handle mouse event clicks. Left mouse sets focus.
            MouseEventKind::Down(mouse_button) => {
                const OFFSET_TO_FIRST_ENTRY: u16 = 1;


                if mouse_button.is_left() {

                    if is_in_hitbox((x, y), &self.split.get_horizontal_hitbox(frame_area)) {
                        self.last_drag = Some(DragKind::HORIZONTAL);
                        return;
                    }

                    if is_in_hitbox((x, y), &self.split.get_vertical_split_hitbox(frame_area)) {
                        self.last_drag = Some(DragKind::VERTICAL);
                        return;
                    }
                    
                    if let Some((menu_screen, popup_area)) = popup_area {

                        if is_in_hitbox((x,y), popup_area) 
                        {
                            match menu_screen {
                                MenuScreenType::EDITOR => { 
                                    self.focus = Focus::EDITOR(EditorFocus::MENU) 
                                } 
                                MenuScreenType::SEARCH => { 
                                    self.focus = Focus::SEARCH 
                                }
                            }
                            
                            return;
                        } else {
                            self.menu_screen = None;
                        }
                    }
  
                    if is_in_hitbox((x, y), editor_area) 
                        && let Some(_) = &self.selected_editor {
                            self.preview_pane = None;
                            self.focus = Focus::EDITOR(EditorFocus::MAIN);
                        
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
                    if is_in_hitbox((x, y), file_area) {
                        let k = (y - OFFSET_TO_FIRST_ENTRY) as usize;

                        let Some(dir) = self.file_system.get_dir_at_index(k) else {
                            return;
                        };

                        let dir = dir.to_path_buf();

                        self.change_dir(&dir);
                    }
                }
            }

            MouseEventKind::ScrollRight => {
                
                if let Some(index) = &mut self.selected_editor {
                    
                    if *index == (self.editor_panes.len() - 1) {
                        *index = 0;
                    } else {
                        *index += 1;
                    }
                    
                };
              
            }

            MouseEventKind::ScrollLeft => {
                
                if let Some(index) = &mut self.selected_editor {
                    
                    if *index == 0 {
                        *index = self.editor_panes.len().saturating_sub(1);
                    } else {
                        *index = index.saturating_sub(1);
                    }
                    
                };
               
            }


            MouseEventKind::ScrollDown => {
                if is_in_hitbox((x,y), shell_area)
                    && ( self.output_pos > 0) {
                   self.output_pos -= 1; 
                } /* else if is_in_hitbox((x,y), file_area) {
                    self.file_system.traverse_dirs(Direction::UP);
                } */
            }

            MouseEventKind::ScrollUp => {
                if is_in_hitbox((x,y), shell_area) 
                    && ((self.output_pos + 1) < (self.output.len() as u16)) {
                   self.output_pos += 1; 
                } /* else if is_in_hitbox((x,y), file_area) {
                    self.file_system.traverse_dirs(Direction::DOWN);
                } */
            }

            _ => { /* Nothing to do. */ }
        }
    }

    fn handle_key_event(&mut self, key_event: KeyEvent, editor_area: &Rect) {
        match key_event.code {
            KeyCode::Esc => {
                
                if let Some(_) = self.preview_pane {
                    self.preview_pane = None;
                    return;
                };

     
                match &self.focus {
                    Focus::SHELL | Focus::FILES => {
                    
                        if let Some(_index) = self.selected_editor {
                            
                            self.menu_screen = Some(MenuScreen::EDITOR(
                                PopupMenu::default()
                                    // TODO: Only add save if contents have changed.
                                    .add_field("Save?".to_owned())
                                    .add_field("Exit?".to_owned()),
                            ));
                            self.focus = Focus::EDITOR(EditorFocus::MENU);

                            return; 
                        };
                        self.exit()
                    }  
                    Focus::SEARCH => {
                        // Cleanup after leaving search
                        if let Some(MenuScreen::SEARCH(popup)) = &mut self.menu_screen {
                            popup.cleanup();
                        };

                        self.menu_screen = None;
                        self.focus = Focus::FILES;
                    }
                    Focus::EDITOR(editor_focus) => match editor_focus {
                        EditorFocus::MAIN => {
                            self.menu_screen = Some(MenuScreen::EDITOR(
                                PopupMenu::default()
                                    // TOOD: Only add save if contents have changed
                                    .add_field("Save?".to_owned())
                                    .add_field("Exit?".to_owned()),
                            ));
                            self.focus = Focus::EDITOR(EditorFocus::MENU);
                        }

                        EditorFocus::MENU => {
                            self.menu_screen = None;
                            self.focus = Focus::EDITOR(EditorFocus::MAIN);
                        }
                    },
                }
            }
            KeyCode::Down | KeyCode::Up => match &self.focus {
                Focus::SEARCH => {
                    if let Some(MenuScreen::SEARCH(popup)) = &mut self.menu_screen {
                        popup.traverse_items(Direction::from(key_event.code));
                    };
                }
                Focus::FILES => {
                    if let Some(file) = self
                        .file_system
                        .traverse_dirs(Direction::from(key_event.code))
                    {
                        self.preview_file(&file);
                    }
                }
                Focus::SHELL => {
                    self.input.handle_event(&Event::Key(key_event));
                }
                Focus::EDITOR(editor_focus) => match editor_focus {
                    EditorFocus::MAIN => {
                        if let Some(index) = self.selected_editor {
                            let _ = self.editor_panes[index].pane.input(key_event, editor_area);
                        };
                    }
                    EditorFocus::MENU => {
                        if let Some(MenuScreen::EDITOR(popup)) = &mut self.menu_screen {
                            popup.traverse_items(Direction::from(key_event.code));
                        };
                    }
                },
            },

            KeyCode::Modifier(_modifiier) => {
                match self.focus {
                    Focus::FILES | Focus::SEARCH => {
                        // TODO: File modifiiers.
                    }
                    Focus::SHELL => {
                        self.input.handle_event(&Event::Key(key_event));
                    }
                    Focus::EDITOR(_) => {
                        if let Some(index) = self.selected_editor {
                                let _ = self.editor_panes[index].pane.input(key_event, editor_area);
                        };
                    }
                }
            }

            KeyCode::Left => {
                
                if ! key_event.modifiers.contains(KeyModifiers::SHIFT)
                {
                    if let Focus::EDITOR(_) = self.focus {
                        if let Some(index) = self.selected_editor {
                            let _ = self.editor_panes[index].pane.input(key_event, editor_area);
                        };
                        return;
                    }
                }

                if let Some(index) = &mut self.selected_editor {
                    
                    if *index == 0 {
                        *index = self.editor_panes.len().saturating_sub(1);
                    } else {
                        *index = index.saturating_sub(1);
                    }
                    
                };
                
            }

            KeyCode::Right => {
                
                if ! key_event.modifiers.contains(KeyModifiers::SHIFT)
                {
                    if let Focus::EDITOR(_) = self.focus {
                        if let Some(index) = self.selected_editor {
                            let _ = self.editor_panes[index].pane.input(key_event, editor_area);
                        };
                        return;
                    }
                }

                if let Some(index) = &mut self.selected_editor {
                    
                    if *index == (self.editor_panes.len() - 1) {
                        *index = 0;
                    } else {
                        *index += 1;
                    }
                    
                };
                
            }

            KeyCode::Tab if self.focus == Focus::FILES => {
                match self.file_system.get_selected() {
                    
                    FileType::Directory(path) => {
                        
                        self.change_dir(&path);

                    }

                    FileType::File(path) => {
                        
                        self.open_file(&path);
                    }

                    FileType::None => {

                    }

                }
            }

            KeyCode::Tab if self.focus == Focus::SEARCH => {
                let mut line_num = 0;
                let new_focus = if let Some(MenuScreen::SEARCH(popup)) = &mut self.menu_screen {
                    let Some(item) = popup.get_selected_item() else {
                        return;
                    };

                    // TODO: Path returns string, should return &Path. This is pretty horrible logic.
                    let path = PathBuf::from(item.metadata().0);

                    if path.is_dir() {
                        self.change_dir(&path);

                        Focus::FILES
                    } else {
                        line_num = item.metadata().1.unwrap_or(1);

                        self.open_file(&path);
                        self.preview_pane = None;

                        Focus::EDITOR(EditorFocus::MAIN)
                    }
                } else {
                    Focus::SEARCH
                };

                if self.focus != new_focus {
                    self.focus = new_focus;
                    self.menu_screen = None;

                    if let Some(index) = self.selected_editor {
                        for _ in 0..(line_num - 1) {
                            self.editor_panes[index].pane.apply(MoveDown { shift: false });
                        }

                        // Force editor refresh.
                        self.editor_panes[index].pane.focus(&editor_area);
                    }
                }
            }

            KeyCode::BackTab => match self.focus {
                Focus::SEARCH => {}
                Focus::FILES => {
                    if let Some(_) = &self.selected_editor {
                        self.focus = Focus::EDITOR(EditorFocus::MAIN);
                    } else {
                        self.focus = Focus::SHELL;
                    };
                }
                Focus::EDITOR(_) => self.focus = Focus::SHELL,
                Focus::SHELL => self.focus = Focus::FILES,
            },

            KeyCode::Enter => match &self.focus {
                Focus::SEARCH => {
                    let cwd = self.shell.borrow().cwd().to_path_buf();

                    if let Some(MenuScreen::SEARCH(popup)) = &mut self.menu_screen {
                        popup.search(&cwd);
                    }
                }
                Focus::FILES => {
                    if let Some(path) = self.file_system.toggle_dir(false) {
                        let target_path = path.to_path_buf();
                        self.open_file(&target_path);

                        self.preview_pane = None;
                        self.focus = Focus::EDITOR(EditorFocus::MAIN);
                    }
                }
                Focus::SHELL => {
                    self.input.handle_event(&Event::Key(key_event));
                }
                Focus::EDITOR(editor_focus) => match editor_focus {
                    EditorFocus::MAIN => {
                        if let Some(index) = self.selected_editor {
                            let _ = self.editor_panes[index].pane.input(key_event, editor_area);
                        };
                    }
                    EditorFocus::MENU => {
                        let mut close_menu = false;
                        if let Some(MenuScreen::EDITOR(popup_menu)) = &mut self.menu_screen {

                            if popup_menu.selected("Save?".to_owned()) {

                                if let Some(index) = self.selected_editor {
                                    let editor = &self.editor_panes[index];

                                    let content = &editor.pane.get_content();

                                    let _ = fs::write(editor.path.clone(), content);
                                       
                                    
                                    let hash = crc64::crc64(0, content.as_bytes()); 
                                    self.editor_panes[index].hash = hash;

                                    close_menu = true;
                                };
                            }

                            if popup_menu.selected("Exit?".to_owned()) {
																
																if let Some(index) = self.selected_editor {
																		self.editor_panes.remove(index);

																		if self.editor_panes.len() == 0 {
																				self.selected_editor = None;
																		} else if self.editor_panes.len() <= index {
																				self.selected_editor = Some(self.editor_panes.len() - 1);
																		}																
																};
                                
                                if ! self.selected_editor.is_some() {
                                    self.focus = Focus::FILES;
                                }

                                close_menu = true;
                            }
                        };

                        if close_menu {
                            self.menu_screen = None;
                        }
                    }
                },
            },

            KeyCode::Char('?') => {
                // TODO: Apply some context about what we should search based on previous focus
                
                let file_path = if let Some(index) = & self.selected_editor {
                    Some(&self.editor_panes[*index].path)
                } else {
                    None
                };
                
                self.menu_screen = Some(MenuScreen::SEARCH(SearchMenu::new(file_path.map(|v| &**v))));
                self.focus = Focus::SEARCH;
            }

            _ => {
                match self.focus {
                    Focus::SEARCH => {
                        if let Some(MenuScreen::SEARCH(popup)) = &mut self.menu_screen {
                            popup.handle_event(&Event::Key(key_event));
                        };
                    }
                    Focus::FILES => {
                        // TODO: Handle other keys
                    }
                    Focus::SHELL => {
                        self.input.handle_event(&Event::Key(key_event));
                    }
                    Focus::EDITOR(_) => {
                        if let Some(index) =self.selected_editor {
                            let _ = self.editor_panes[index].pane.input(key_event, editor_area);
                        };
                    }
                }
            }
        }
    }
}
