use crossterm::event::KeyEvent;

pub trait Interactible {
    fn handle_key_event(&mut self, key_event: &KeyEvent) -> bool;
    fn set_focus(&mut self, focus: bool);
}
