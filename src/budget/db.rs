use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::PathBuf;

use crate::budget::Transaction;
use crate::budget::data::Categories;
use crate::db::DB;

pub struct Transactions;

pub fn init(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "BEGIN;
        CREATE TABLE IF NOT EXISTS transactions (
            id INTEGER PRIMARY KEY,
            identifier TEXT NOT NULL UNIQUE,
            date DATETIME NOT NULL,
            amount_in_cents INTEGER,
            account_id TEXT,
            category_name TEXT,
            description TEXT,
            merchant TEXT
        );
        COMMIT;",
    )?;
    Ok(())
}

impl Transactions {
    fn write_transaction(conn: &Connection, transaction: &Transaction) -> rusqlite::Result<i64> {
        conn.execute(
            "INSERT OR IGNORE INTO transactions(date, account_id, amount_in_cents, category_name, description, merchant, identifier) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![&transaction.date, transaction.account_id, transaction.amount_in_cents,
                transaction.category.name, transaction.description , transaction.merchant, transaction.identifier],
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn load_data_files(folder_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let mut db = DB
            .get()
            .ok_or_else(|| rusqlite::Error::InvalidQuery)?
            .lock()
            .unwrap();
        // list csv files in the folder
        for csv_file in std::fs::read_dir(folder_path)? {
            let csv_file = csv_file?;
            if let Some(extension) = csv_file.path().extension() {
                if extension == "csv" {
                    // load transactions from the csv file
                    // let transactions = data::load_transactions_from_csv(&csv_file.path())?;
                    // db::Transactions::write(&mut db.conn, &transactions)?;
                }
            }
        }
        Ok(())
    }

    pub fn write(conn: &mut Connection, transactions: &[Transaction]) -> rusqlite::Result<()> {
        let tx = conn.transaction()?;

        for transaction in transactions {
            Transactions::write_transaction(&tx, transaction)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn read_all() -> rusqlite::Result<Vec<Transaction>> {
        let db = DB
            .get()
            .ok_or_else(|| rusqlite::Error::InvalidQuery)?
            .lock()
            .unwrap();
        let mut stmt = db.conn.prepare(
            "SELECT id, identifier, date, amount_in_cents, account_id, category_name, description, merchant
            FROM transactions
            ORDER BY date DESC",
        )?;
        let transactions = stmt
            .query_map([], |r| {
                Ok(Transaction {
                    db_id: Some(r.get::<_, i64>(0)?),
                    identifier: r.get(1)?,
                    date: r.get::<_, DateTime<Utc>>(2)?,
                    amount_in_cents: r.get(3)?,
                    account_id: r.get(4)?,
                    category: {
                        let category_name: String = r.get(5)?;
                        Categories::get_or_uncategorized(category_name.as_str())
                    },
                    description: r.get(6)?,
                    merchant: r.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(transactions)
    }

    pub fn read_month(year: u32, month: u32) -> rusqlite::Result<Vec<Transaction>> {
        let db = DB
            .get()
            .ok_or_else(|| rusqlite::Error::InvalidQuery)?
            .lock()
            .unwrap();
        // 1. Calculate the start date of the requested month
        let start_date = format!("{:04}-{:02}-01 00:00:00", year, month);

        // 2. Calculate the start date of the *next* month for our exclusive upper bound
        let (next_year, next_month) = if month == 12 {
            (year + 1, 1)
        } else {
            (year, month + 1)
        };
        let end_date = format!("{:04}-{:02}-01 00:00:00", next_year, next_month);

        // 3. Prepare and execute the query
        let mut stmt = db.conn.prepare(
            "SELECT id, identifier, date, amount_in_cents, account_id, category_name, description, merchant
                 FROM transactions
                 WHERE date >= ?1 AND date < ?2
                 ORDER BY date DESC",
        )?;

        let transaction_iter = stmt.query_map(params![start_date, end_date], |row| {
            Ok(Transaction {
                db_id: row.get(0)?,
                identifier: row.get(1)?,
                date: row.get(2)?,
                amount_in_cents: row.get(3)?,
                account_id: row.get(4)?,
                category: {
                    let category_name: String = row.get(5)?;
                    Categories::get_or_uncategorized(category_name.as_str())
                },
                description: row.get(6)?,
                merchant: row.get(7)?,
            })
        })?;

        // 4. Collect the results into a Vector
        let mut transactions = Vec::new();
        for tx in transaction_iter {
            transactions.push(tx?);
        }

        Ok(transactions)
    }

    pub fn available_months() -> rusqlite::Result<Vec<(u32, u32)>> {
        let db = DB
            .get()
            .ok_or_else(|| rusqlite::Error::InvalidQuery)?
            .lock()
            .unwrap();
        let mut stmt = db.conn.prepare(
            "SELECT DISTINCT
                CAST(strftime('%Y', date) AS INTEGER),
                CAST(strftime('%m', date) AS INTEGER)
             FROM transactions
             WHERE date IS NOT NULL
             ORDER BY 1 DESC, 2 DESC",
        )?;

        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect()
    }

    pub fn last_entry_date_for_account(
        conn: &Connection,
        account_identifier: &str,
    ) -> rusqlite::Result<Option<DateTime<Utc>>> {
        let res: rusqlite::Result<DateTime<Utc>> = conn.query_row(
            "SELECT MAX(date) FROM transactions WHERE account_id = ?1",
            params![account_identifier],
            |r| r.get(0),
        );
        res.optional()
    }

    pub fn oldest_entry_date_for_account(
        conn: &Connection,
        account_identifier: &str,
    ) -> rusqlite::Result<Option<DateTime<Utc>>> {
        let res: rusqlite::Result<DateTime<Utc>> = conn.query_row(
            "SELECT MIN(date) FROM transactions WHERE account_id = ?1",
            params![account_identifier],
            |r| r.get(0),
        );
        res.optional()
    }

    pub fn update_transaction(transaction: &Transaction) -> rusqlite::Result<()> {
        let db = DB
            .get()
            .ok_or_else(|| rusqlite::Error::InvalidQuery)?
            .lock()
            .unwrap();
        db.conn.execute(
            "UPDATE transactions SET category_name = ?1 WHERE identifier = ?2",
            params![transaction.category.name, transaction.identifier],
        )?;
        Ok(())
    }

    pub fn update_transactions(transactions: &[Transaction]) -> rusqlite::Result<()> {
        let mut db = DB
            .get()
            .ok_or_else(|| rusqlite::Error::InvalidQuery)?
            .lock()
            .unwrap();

        let tx = db.conn.transaction()?;

        // 2. Prepare the statement once (much faster than parsing SQL in a loop)
        {
            let mut stmt =
                tx.prepare("UPDATE transactions SET category_name = ?1 WHERE identifier = ?2")?;

            for transaction in transactions.iter() {
                stmt.execute(params![transaction.category.name, transaction.identifier])?;
            }
        } // stmt is dropped here so we can commit the transaction

        // 3. Commit the changes
        tx.commit()?;
        Ok(())
    }
}

// pub fn update_transaction_category(
//     conn: &Connection,
//     transaction_id: i64,
//     category: &str,
//     subcategory: Option<&str>,
// ) -> rusqlite::Result<()> {
//     let category = category.trim();
//     let mut category_id: Option<i64> = None;
//     let mut subcategory_text: Option<String> = None;

//     if !category.is_empty() {
//         let cat_id = get_or_create_category(conn, category, None)?;
//         if let Some(sub) = subcategory.filter(|s| !s.trim().is_empty()) {
//             let sub_trimmed = sub.trim();
//             // Ensure subcategory exists under the parent, but keep category_id as parent
//             let _sub_id = get_or_create_category(conn, sub_trimmed, Some(cat_id))?;
//             category_id = Some(cat_id);
//             subcategory_text = Some(sub_trimmed.to_string());
//         } else {
//             category_id = Some(cat_id);
//             subcategory_text = None;
//         }
//     } else if let Some(sub) = subcategory.filter(|s| !s.trim().is_empty()) {
//         let sub_trimmed = sub.trim();
//         let sub_id = get_or_create_category(conn, sub_trimmed, None)?;
//         category_id = Some(sub_id);
//         subcategory_text = None;
//     }

//     conn.execute(
//         "UPDATE transactions SET category_id = ?1, subcategory = ?2 WHERE id = ?3",
//         params![category_id, subcategory_text, transaction_id],
//     )?;
//     Ok(())
// }
