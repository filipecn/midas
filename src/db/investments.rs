use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};

use crate::db::DB;

pub struct Investments;
pub struct InvestmentTransactions;

fn save_original_amount_override(
    conn: &Connection,
    identifier: &str,
    amount_in_cents: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO investment_original_amount_overrides(
            investment_identifier, amount_original_in_cents
        ) VALUES (?1, ?2)
        ON CONFLICT(investment_identifier) DO UPDATE SET
            amount_original_in_cents = excluded.amount_original_in_cents",
        params![identifier, amount_in_cents],
    )?;
    Ok(())
}

pub fn init(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "BEGIN;
                CREATE TABLE IF NOT EXISTS investments (
            id INTEGER PRIMARY KEY,
            identifier TEXT NOT NULL UNIQUE,
            date DATETIME NOT NULL,
            due_date DATETIME,
            name TEXT NOT NULL,
            amount_in_cents INTEGER,
            amount_original_in_cents INTEGER,
            investment_type TEXT,
            subtype TEXT,
            status TEXT,
            rate REAL,
            rate_type TEXT,
            rate_periodicity TEXT,
            fixed_annual_rate REAL
            );
        CREATE TABLE IF NOT EXISTS investment_transactions (
            id INTEGER PRIMARY KEY,
            identifier TEXT NOT NULL UNIQUE,
            investment_id TEXT,
            transaction_type TEXT,
            quantity REAL,
            value_in_cents INTEGER,
            amount_in_cents INTEGER,
            date DATETIME,
            trade_date DATETIME,
            price_per_unit_in_cents INTEGER
        );
        CREATE TABLE IF NOT EXISTS investment_original_amount_overrides (
            investment_identifier TEXT PRIMARY KEY NOT NULL,
            amount_original_in_cents INTEGER NOT NULL
        );
        COMMIT;",
    )?;
    Ok(())
}

impl Investments {
    pub fn update_original_amount(identifier: &str, amount_in_cents: i64) -> rusqlite::Result<()> {
        let db = DB
            .get()
            .ok_or(rusqlite::Error::InvalidQuery)?
            .lock()
            .unwrap();
        save_original_amount_override(&db.conn, identifier, amount_in_cents)
    }

    pub fn write_investment(
        conn: &Connection,
        investment: &crate::investments::Investment,
    ) -> rusqlite::Result<i64> {
        conn.execute(
            "INSERT INTO investments(date, due_date, name, amount_in_cents, amount_original_in_cents, investment_type, subtype, identifier, status, rate, rate_type, rate_periodicity, fixed_annual_rate)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(identifier) DO UPDATE SET
                 date = excluded.date,
                 due_date = excluded.due_date,
                 name = excluded.name,
                 amount_in_cents = excluded.amount_in_cents,
                 amount_original_in_cents = excluded.amount_original_in_cents,
                 investment_type = excluded.investment_type,
                 subtype = excluded.subtype,
                 status = excluded.status,
                 rate = excluded.rate,
                 rate_type = excluded.rate_type,
                 rate_periodicity = excluded.rate_periodicity,
                 fixed_annual_rate = excluded.fixed_annual_rate",
            params![&investment.date, investment.due_date, investment.name,
                investment.amount_in_cents, investment.amount_original_in_cents,
                investment.investment_type, investment.subtype, investment.identifier,
                investment.status,
                investment.rate, investment.rate_type, investment.rate_periodicity,
                investment.fixed_annual_rate],
        )?;
        conn.query_row(
            "SELECT id FROM investments WHERE identifier = ?1",
            [&investment.identifier],
            |row| row.get(0),
        )
    }

    pub fn write(
        conn: &mut Connection,
        investments: &[crate::investments::Investment],
    ) -> rusqlite::Result<()> {
        let tx = conn.transaction()?;

        for investment in investments {
            Investments::write_investment(&tx, investment)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn read_all() -> rusqlite::Result<Vec<crate::investments::Investment>> {
        let db = DB
            .get()
            .ok_or_else(|| rusqlite::Error::InvalidQuery)?
            .lock()
            .unwrap();
        let mut stmt = db.conn.prepare(
            "SELECT investment.id, investment.identifier, investment.date, investment.due_date,
                    investment.name, investment.amount_in_cents,
                    COALESCE(amount_override.amount_original_in_cents, investment.amount_original_in_cents),
                    investment.investment_type, investment.subtype, investment.status, investment.rate,
                    investment.rate_type, investment.rate_periodicity, investment.fixed_annual_rate
            FROM investments AS investment
            LEFT JOIN investment_original_amount_overrides AS amount_override
                ON amount_override.investment_identifier = investment.identifier
            ORDER BY investment.date DESC",
        )?;
        let investments = stmt
            .query_map([], |r| {
                Ok(crate::investments::Investment {
                    db_id: Some(r.get::<_, i64>(0)?),
                    identifier: r.get(1)?,
                    date: r.get::<_, DateTime<Utc>>(2)?,
                    due_date: r.get(3)?,
                    name: r.get(4)?,
                    amount_in_cents: r.get(5)?,
                    amount_original_in_cents: r.get(6)?,
                    investment_type: r.get::<_, Option<String>>(7)?.unwrap_or_default(),
                    subtype: r.get::<_, Option<String>>(8)?.unwrap_or_default(),
                    status: r.get::<_, Option<String>>(9)?.unwrap_or_default(),
                    rate: r.get(10)?,
                    rate_type: r.get(11)?,
                    rate_periodicity: r.get(12)?,
                    fixed_annual_rate: r.get(13)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(investments)
    }
}

impl InvestmentTransactions {
    pub fn write_investment_transaction(
        conn: &Connection,
        investment_transaction: &crate::investments::InvestmentTransaction,
    ) -> rusqlite::Result<i64> {
        conn.execute(
            "INSERT INTO investment_transactions(
                date, investment_id, transaction_type, amount_in_cents, identifier,
                price_per_unit_in_cents, quantity, value_in_cents
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(identifier) DO UPDATE SET
                date = excluded.date,
                investment_id = excluded.investment_id,
                transaction_type = excluded.transaction_type,
                amount_in_cents = excluded.amount_in_cents,
                price_per_unit_in_cents = excluded.price_per_unit_in_cents,
                quantity = excluded.quantity,
                value_in_cents = excluded.value_in_cents",
            params![
                &investment_transaction.date,
                investment_transaction.investment_id,
                investment_transaction.transaction_type.to_string(),
                investment_transaction.amount_in_cents,
                investment_transaction.identifier,
                investment_transaction.price_per_unit_in_cents,
                investment_transaction.quantity,
                investment_transaction.value_in_cents,
            ],
        )?;
        conn.query_row(
            "SELECT id FROM investment_transactions WHERE identifier = ?1",
            [&investment_transaction.identifier],
            |row| row.get(0),
        )
    }

    pub fn write(
        conn: &mut Connection,
        investment_transactions: &[crate::investments::InvestmentTransaction],
    ) -> rusqlite::Result<()> {
        let tx = conn.transaction()?;

        for investment_transaction in investment_transactions {
            InvestmentTransactions::write_investment_transaction(&tx, investment_transaction)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn read_all() -> rusqlite::Result<Vec<crate::investments::InvestmentTransaction>> {
        let db = DB
            .get()
            .ok_or_else(|| rusqlite::Error::InvalidQuery)?
            .lock()
            .unwrap();
        let mut stmt = db.conn.prepare(
            "SELECT id, identifier, date, investment_id, transaction_type, amount_in_cents,
                    price_per_unit_in_cents, quantity, value_in_cents
            FROM investment_transactions
            ORDER BY date DESC",
        )?;
        let investment_transactions = stmt
            .query_map([], |r| {
                Ok(crate::investments::InvestmentTransaction {
                    db_id: Some(r.get::<_, i64>(0)?),
                    identifier: r.get(1)?,
                    date: r.get::<_, DateTime<Utc>>(2)?,
                    investment_id: r.get(3)?,
                    transaction_type: crate::investments::InvestmentTransactionType::from(
                        r.get::<_, String>(4)?,
                    ),
                    amount_in_cents: r.get(5)?,
                    price_per_unit_in_cents: r.get(6)?,
                    quantity: r.get(7)?,
                    value_in_cents: r.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(investment_transactions)
    }
}

#[cfg(test)]
mod tests {
    use super::{Investments, init, save_original_amount_override};
    use crate::investments::Investment;
    use chrono::Utc;
    use rusqlite::Connection;

    #[test]
    fn original_amount_override_survives_investment_refresh() {
        let conn = Connection::open_in_memory().unwrap();
        init(&conn).unwrap();
        let mut investment = Investment {
            db_id: None,
            identifier: "investment-1".to_string(),
            date: Utc::now(),
            due_date: None,
            name: "Investment".to_string(),
            amount_in_cents: 15_000,
            amount_original_in_cents: 10_000,
            investment_type: "fixed income".to_string(),
            subtype: "bond".to_string(),
            status: "active".to_string(),
            rate: None,
            rate_type: None,
            rate_periodicity: None,
            fixed_annual_rate: None,
        };
        Investments::write_investment(&conn, &investment).unwrap();
        save_original_amount_override(&conn, &investment.identifier, 12_500).unwrap();

        investment.amount_original_in_cents = 10_000;
        Investments::write_investment(&conn, &investment).unwrap();

        let overridden_amount: i64 = conn
            .query_row(
                "SELECT COALESCE(amount_override.amount_original_in_cents, investment.amount_original_in_cents)
                FROM investments AS investment
                LEFT JOIN investment_original_amount_overrides AS amount_override
                    ON amount_override.investment_identifier = investment.identifier
                WHERE investment.identifier = ?1",
                [&investment.identifier],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(overridden_amount, 12_500);
    }
}
