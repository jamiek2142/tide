use std::
    {
        default, ffi::OsString, io, thread::sleep, time::Duration
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

pub struct App
{
    input : String,
    character_index : usize,
    exit  : bool,
    output : Vec<String>,
    tx   : Sender<Vec<u8>>,
    rx   : Receiver<Vec<u8>>
}

impl App
{
    pub fn new () -> Self
    {
        let (tx, rx) = unbounded::<Vec<u8>>();

        Self 
        { 
            input: String::default(), 
            character_index: usize::default(), 
            exit: bool::default(), 
            output: Vec::new(), 
            tx : tx,
            rx : rx,

        }
    }

    pub fn run (&mut self, terminal: &mut DefaultTerminal) -> io::Result<()>
    { 
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
       let pty_system = NativePtySystem::default(); 
       let pair       = pty_system.openpty(PtySize::default()).unwrap();

       let argv = self.input
           .split_whitespace()
           .map(OsString::from)
           .collect();

       let cmd = CommandBuilder::from_argv(argv);

       let mut _child = pair.slave.spawn_command(cmd).unwrap();
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

       self.clear();
    }

    fn exit (&mut self)
    {
        self.exit = true;
    }

    fn draw (&self, frame : &mut Frame)
    {
        let layout = Layout::vertical([
                                 Constraint::Fill(21), 
                                 Constraint::Min(1)
                              ]);

        let [output_area, input_area] = frame.area().layout(&layout);
    
        let input = Paragraph::new(self.input.as_str())
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

        frame.render_widget(input, input_area);
        frame.render_widget(output, output_area);
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
 
    fn clamp_cursor (&self, new_cursor_pos : usize) -> usize
    {
        new_cursor_pos.clamp(0, self.input.chars().count())
    }

    fn byte_index (&self) -> usize
    {
        self.input
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.character_index)
            .unwrap_or(self.input.len())
    }

    fn move_cursor_right (&mut self)
    {
        let cursor_moved_right = self.character_index.saturating_add(1);
        self.character_index = self.clamp_cursor(cursor_moved_right);
    }

    fn move_cursor_left (&mut self)
    {
        let cursor_moved_left = self.character_index.saturating_sub(1);
        self.character_index = self.clamp_cursor(cursor_moved_left);
    }

    fn enter_char (&mut self, new_char : char)
    {
        let index = self.byte_index();
        self.input.insert(index, new_char);
        self.move_cursor_right();
    }

    fn delete_char (&mut self)
    {
        if self.character_index != 0
        {
            let current_index = self.character_index;

            let from_left_to_current_index = current_index - 1;

            let before_char_to_delete = self.input.chars().take(from_left_to_current_index);
            let after_char_to_delete = self.input.chars().skip(current_index);
            
            self.input = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    fn clear (&mut self)
    {
        self.input.clear();
        self.character_index = 0;
    }

    fn handle_key_event (&mut self, key_event : KeyEvent) 
    {
        match key_event.code {
            KeyCode::Esc => self.exit(),
            KeyCode::Char(to_insert) => self.enter_char(to_insert),
            KeyCode::Backspace => self.delete_char(),
            KeyCode::Enter => self.send_cmd(),
            _ => {}
        }
        
    }

}

fn main () -> io::Result<()> 
{
    ratatui::run(|terminal| App::new().run(terminal))
}
