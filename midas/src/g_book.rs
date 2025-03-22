use dionysus::finance::Book;

#[derive(Default)]
pub struct BookGraph {
    pub book: Book,
    pub x_pos: f64,
}

impl BookGraph {
    pub fn set_book(&mut self, book: &Book) {
        self.book = book.clone();
    }
}
