use std::
    {
        collections::HashMap, default, env, ffi::OsString, io, path::{
            Path, PathBuf
        }, thread::sleep, time::Duration, process::Command
    };

use color_eyre::owo_colors::colors::Default;
use crossterm::event::{
    self, 
    Event, 
    KeyCode,
    KeyEvent, 
    KeyEventKind
};

use crossbeam_channel::{
    unbounded,
    Receiver, 
    Sender
};

use portable_pty::{
    CommandBuilder,
    NativePtySystem,
    PtySize,
    PtyPair,
    PtySystem
};

use tui_input::{
    Input,
    backend::{
        crossterm::{
            EventHandler
        }
    }
};

use ratatui::{
    DefaultTerminal, Frame, buffer::Buffer, layout::{self, Constraint, Layout, Rect}, style::{
        Style, 
        Stylize
    }, symbols::border, text::{
        Line,
        Text
    }, widgets::{
        Block,
        Paragraph,
        Widget
    }
};

use ansi_to_tui::IntoText as _;

#[derive(Default)]
struct ShellState
{
    cwd : PathBuf,
    env : HashMap<String, String>
}

pub struct App
{
    input : Input,
		file_system : String,
    shell_state : ShellState,
    exit  : bool,
    output : Vec<String>,
    tx   : Sender<Vec<u8>>,
    rx   : Receiver<Vec<u8>>
}

impl ShellState
{
    pub fn new() -> Self
    {
        Self 
        { 
            cwd: env::current_dir().unwrap_or_default(), 
            env: HashMap::new()
        }
    }
    
}

impl App
{
    pub fn new () -> Self
    {
        let (tx, rx) = unbounded::<Vec<u8>>();

        Self 
        { 
            input: Input::default(),
						file_system : String::default(), 
            shell_state : ShellState::new(), 
            exit: bool::default(), 
            output: Vec::new(), 
            tx : tx,
            rx : rx,

        }
    }

    pub fn run (&mut self, terminal: &mut DefaultTerminal) -> io::Result<()>
    {	
				self.update_file_system();
 
        while !self.exit
        {
            while let Ok(bytes) = self.rx.try_recv() {
                let text = String::from_utf8_lossy(&bytes);

                self.output.push(text.to_string());
            }

            terminal.draw(|frame| self.draw(frame))?;
            
            self.handle_events()?;
        }
        Ok(())
    }

    fn send_cmd (&mut self)
    {
       /* Clear the output pane. */
       self.clear_output();

       /* Create a PTY each command. */
       let pty_system = NativePtySystem::default(); 
       let pair       = pty_system.openpty(PtySize::default()).unwrap();

       let argv = self.input
           .value()
           .split_whitespace()
           .map(OsString::from)
           .collect();

       let mut cmd = CommandBuilder::from_argv(argv);
       cmd.cwd(&self.shell_state.cwd);
       // TODO: Environment variables.

       let Ok(mut _child) = pair.slave.spawn_command(cmd) else {
           self.output.push("Unknown command".to_string());
           // TODO: Optional clear input.
           return
       };
       let mut reader = pair.master.try_clone_reader().unwrap();
    
       let tx = self.tx.clone();

       std::thread::spawn(move || { 
            let mut buffer = [0u8; 1024];

            while let Ok(n) = reader.read(&mut buffer) {
                if n == 0
                {
                    break;
                }
                let _ = tx.send(buffer[..n].to_vec());
            }
        });
    
       /* Clean out the command buffer */ 
       self.clear_input();

    }

    fn exit (&mut self)
    {
        self.exit = true;
    }

    fn draw (&self, frame : &mut Frame)
    {
        let main_layout = Layout::horizontal([
                                 Constraint::Percentage(30), 
                                 Constraint::Percentage(70)
                               ]).split(frame.area());
        let sub_layout = Layout::vertical([
                                 Constraint::Fill(21), 
                                 Constraint::Min(1)
                               ]).split(main_layout[1]);

        let input = Paragraph::new(self.input.value())
            .style(Style::default())
            .block(Block::bordered().title("Input"));
         
        let text :Text = self.output
            .clone()
            .join("")
            .into_bytes()
            .into_text()
            .unwrap_or_default();
            
        let output = Paragraph::new(text)
            .style(Style::default())
            .block(Block::bordered().title("Output"));
		
        let files = Paragraph::new(self.file_system.as_str())
            .style(Style::default())
            .block(Block::bordered().title("File System"));

        frame.render_widget(files, main_layout[0]);
        frame.render_widget(input, sub_layout[1]);
        frame.render_widget(output, sub_layout[0]);
    }

    fn handle_events (&mut self) -> io::Result<()>
    {
        if event::poll(Duration::from_millis(10))?
        {
            match event::read()?
            {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    self.handle_key_event(key_event)
                }
                _ => {
                    /* Nothing to do. */
                }
            }
        }
        Ok(())
    }
   
    fn update_file_system (&mut self)
    { 
			self.file_system = String::from_utf8_lossy(&Command::new("ls")
                    .current_dir(&self.shell_state.cwd)            
										.output()
									  .unwrap()
										.stdout).to_string();
		}

    fn execute(&mut self)
    {
        let argv : Vec<&str> = self.input
                    .value()
                    .trim()
                    .split_whitespace()
                    .collect();

        match argv[0] 
        {
            "cd" => {
                if argv.len() > 1 {
                    let path_arg = PathBuf::from(argv[1]); 

										let target_path = if path_arg.is_absolute() {
																				path_arg
																			} else {
																				self.shell_state.cwd.join(path_arg)
																			};
	                   
										 // TODO: Handle invalid paths.   
										 self.clear_output();
                     self.shell_state.cwd = std::fs::canonicalize(&target_path)
																							.unwrap_or(self.shell_state.cwd.clone());
                     self.update_file_system();
                     self.clear_input();
                    
                }
            }

            _ => {
                self.send_cmd();   
            } 
        }
    }

    fn clear_output(&mut self)
    {
        self.output.clear();
    }

    fn clear_input (&mut self)
    {
        self.input.reset();
     }

    fn handle_key_event (&mut self, key_event : KeyEvent) 
    {
        match key_event.code {
            KeyCode::Esc => self.exit(),
         // KeyCode::Tab => self.autocomplete(),
            KeyCode::Enter => self.execute(),
            _ => {
                self.input.handle_event(&Event::Key(key_event));
            }
        }
        
    }

}

fn main () -> io::Result<()> 
{
    ratatui::run(|terminal| App::new().run(terminal))
}
