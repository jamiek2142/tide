use std::{io, thread::sleep};

use crossterm::event::{
    self, 
    Event, 
    KeyCode,
    KeyEvent, 
    KeyEventKind
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

#[derive(Debug, Default)]
pub struct App
{
    input : String,
    character_index : usize,
    exit  : bool
}


impl App
{
    pub fn run (&mut self, terminal: &mut DefaultTerminal) -> io::Result<()>
    {
        while !self.exit
        {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
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
            
        let output = Paragraph::new("Output will go here")
            .style(Style::default())
            .block(Block::bordered().title("Output"));

        frame.render_widget(input, input_area);
        frame.render_widget(output, output_area);
    }

    fn handle_events (&mut self) -> io::Result<()>
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

    fn handle_key_event (&mut self, key_event : KeyEvent) 
    {
        match key_event.code {
            KeyCode::Esc => self.exit(),
            KeyCode::Char(to_insert) => self.enter_char(to_insert),
            KeyCode::Backspace => self.delete_char(),
            _ => {}
        }
        
    }

}

fn main () -> io::Result<()> 
{
    ratatui::run(|terminal| App::default().run(terminal))
}
