#![allow(warnings)]

use std::{
    cmp::Ordering,
    collections::HashMap,
    default, env,
    ffi::OsString,
    io,
    path::{Path, PathBuf},
    process::Command,
    thread::sleep,
    time::Duration,
};

use color_eyre::owo_colors::colors::Default;

use walkdir::{DirEntry, WalkDir};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};

use crossbeam_channel::{Receiver, Sender, unbounded};

use portable_pty::{CommandBuilder, NativePtySystem, PtyPair, PtySize, PtySystem};

use tui_input::{Input, backend::crossterm::EventHandler};

use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{self, Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Widget},
};

use ansi_to_tui::IntoText as _;

#[derive(Default)]
struct ShellState {
    cwd: PathBuf,
    env: HashMap<String, String>,
}

enum Direction {
    UP,
    DOWN,
    LEFT,
    RIGHT,
}

#[derive(Default)]
struct FilePath {
    path: String,
    is_dir: bool,
}

#[derive(Default)]
struct FileSystem {
    current_dir_to_render: String,
    paths_to_render: Vec<FilePath>,
    paths_to_objects: Vec<PathBuf>,
}

pub struct App {
    input: Input,
    file_system: FileSystem,
    file_system_state: ListState,
    depth_limit: usize,
    shell_state: ShellState,
    exit: bool,
    output: Vec<String>,
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
}

impl FileSystem {
    pub fn clear(&mut self) {
        self.paths_to_render.clear();
        self.paths_to_objects.clear();
    }
}

impl ShellState {
    pub fn new() -> Self {
        Self {
            cwd: env::current_dir().unwrap_or_default(),
            env: HashMap::new(),
        }
    }
}

impl FilePath {
    pub fn new(path: String, is_dir: bool) -> Self {
        let path = if is_dir { path + "/" } else { path };

        Self {
            path: path,
            is_dir: is_dir,
        }
    }
}

impl App {
    pub fn new() -> Self {
        let (tx, rx) = unbounded::<Vec<u8>>();

        Self {
            input: Input::default(),
            file_system: FileSystem::default(),
            file_system_state: ListState::default(),
            depth_limit: 1,
            shell_state: ShellState::new(),
            exit: bool::default(),
            output: Vec::new(),
            tx: tx,
            rx: rx,
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

            self.handle_events()?;
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
        cmd.cwd(&self.shell_state.cwd);
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
        let main_layout =
            Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)])
                .split(frame.area());
        let sub_layout =
            Layout::vertical([Constraint::Fill(21), Constraint::Min(1)]).split(main_layout[1]);

        let input = Paragraph::new(self.input.value())
            .style(Style::default())
            .block(Block::bordered().title("Input"));

        let text: Text = self
            .output
            .clone()
            .join("")
            .into_bytes()
            .into_text()
            .unwrap_or_default();

        let output = Paragraph::new(text)
            .style(Style::default())
            .block(Block::bordered().title("Output"));

        let items: Vec<ListItem> = self
            .file_system
            .paths_to_render
            .iter()
            .map(|k| {
                let style = if k.is_dir {
                    Style::default()
                        .fg(Color::LightMagenta)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(k.path.as_str()).style(style)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    // .borders(Borders::ALL)
                    .title(self.file_system.current_dir_to_render.as_str()),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Yellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        frame.render_stateful_widget(list, main_layout[0], &mut self.file_system_state);

        frame.render_widget(input, sub_layout[1]);
        frame.render_widget(output, sub_layout[0]);
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if event::poll(Duration::from_millis(10))? {
            match event::read()? {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    self.handle_key_event(key_event)
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
        self.file_system_state.select(Some(0));

        let walker = WalkDir::new(&self.shell_state.cwd).sort_by(|a, b| {
            let a_is_dir = a.file_type().is_dir();
            let b_is_dir = b.file_type().is_dir();

            match (a_is_dir, b_is_dir) {
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                _ => a.file_name().cmp(b.file_name()),
            }
        });

        let mut path_to_upper_dir = self.shell_state.cwd.clone();

        self.file_system.current_dir_to_render = self.shell_state.cwd.to_string_lossy().to_string();

        path_to_upper_dir.push("..");

        let parent_dir = self
            .shell_state
            .cwd
            .parent()
            .map_or(String::default(), |x| {
                x.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            });

        // TODO: Fix for root paths.
        self.file_system.current_dir_to_render = "../".to_string()
            + &parent_dir
            + "/"
            + &self
                .shell_state
                .cwd
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();

        self.file_system.paths_to_objects.push(path_to_upper_dir);
        self.file_system
            .paths_to_render
            .push(FilePath::new("..".to_string(), true));

        for entry in walker.into_iter().filter_map(|e| e.ok()) {
            let depth = entry.depth();

            // TODO: Recursive depth limit set by left right keys
            if (depth > self.depth_limit) || (depth == 0) || self.is_dotfile(&entry) {
                continue;
            }

            let prefix = "  ".repeat(depth - 1);

            self.file_system.paths_to_render.push(FilePath::new(
                prefix + &entry.file_name().to_string_lossy(),
                entry.path().is_dir(),
            ));

            self.file_system.paths_to_objects.push(entry.into_path());
        }
    }

    fn change_dir(&mut self, target_path: &PathBuf) {
        // TODO: Handle invalid paths.
        self.clear_output();
        self.shell_state.cwd =
            std::fs::canonicalize(&target_path).unwrap_or(self.shell_state.cwd.clone());
        self.update_file_system();
        self.clear_input();
    }

    fn open_file(&mut self, target_path: &PathBuf) {
        let path = &target_path.to_string_lossy();
        let cmd = vec!["cat", path];

        self.send_cmd(cmd);
    }

    fn execute(&mut self) {
        let input = self.input.clone();
        let argv: Vec<&str> = input.value().trim().split_whitespace().collect();

        if argv.len() == 0 {
            let Some(file_index) = self.file_system_state.selected() else {
                return;
            };

            let target_path = self.file_system.paths_to_objects[file_index].clone();
            self.output.push(target_path.to_string_lossy().to_string());

            if target_path.is_dir() {
                self.change_dir(&target_path);
            } else if target_path.is_file() {
                self.open_file(&target_path);
            }

            return;
        }

        match argv[0] {
            "cd" => {
                if argv.len() > 1 {
                    let path_arg = PathBuf::from(argv[1]);

                    let target_path = if path_arg.is_absolute() {
                        path_arg
                    } else {
                        self.shell_state.cwd.join(path_arg)
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
            Direction::UP => match self.file_system_state.selected() {
                Some(k) => {
                    if k <= 0 {
                        self.file_system.paths_to_render.len() - 1
                    } else {
                        k - 1
                    }
                }
                None => 0,
            },
            Direction::DOWN => match self.file_system_state.selected() {
                Some(k) => {
                    if k >= self.file_system.paths_to_render.len() - 1 {
                        0
                    } else {
                        k + 1
                    }
                }
                None => 0,
            },
            Direction::LEFT => {
                if self.depth_limit > 1 {
                    self.depth_limit = self.depth_limit - 1;
                    self.update_file_system();
                }
                return;
            }
            Direction::RIGHT => {
                self.depth_limit = self.depth_limit + 1;
                self.update_file_system();
                return;
            }
        };

        self.file_system_state.select(Some(k));
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => self.exit(),
            KeyCode::Down => self.traverse_dirs(Direction::DOWN),
            KeyCode::Up => self.traverse_dirs(Direction::UP),
            KeyCode::Right => self.traverse_dirs(Direction::RIGHT),
            KeyCode::Left => self.traverse_dirs(Direction::LEFT),
            // KeyCode::Tab => self.autocomplete(),
            KeyCode::Enter => self.execute(),
            _ => {
                self.input.handle_event(&Event::Key(key_event));
            }
        }
    }
}

fn main() -> io::Result<()> {
    ratatui::run(|terminal| App::new().run(terminal))
}
