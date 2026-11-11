use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::PathBuf;
use std::sync::OnceLock;

use crate::budget::Transaction;
use crate::budget::data::Categories;

pub static DB: OnceLock<std::sync::Mutex<DBConnection>> = OnceLock::new();

pub struct DBConnection {
    pub conn: Connection,
}

impl DBConnection {
    pub fn new(path: &String) -> rusqlite::Result<Self> {
        let db = Self {
            conn: Connection::open(&path)?,
        };
        // investment_type TEXT,
        // due_date DATETIME,
        // updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        Ok(db)
    }
}
