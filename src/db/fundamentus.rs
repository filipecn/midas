use rusqlite::Connection;

use crate::{db::DB, fundamentus::StockDetails};

pub struct Stocks;

pub fn init(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "BEGIN;
        CREATE TABLE IF NOT EXISTS stock_details (
            id INTEGER PRIMARY KEY,
            ticker TEXT NOT NULL UNIQUE,
            nome TEXT,
            cotacao TEXT,
            data_cot TEXT,
            tipo TEXT,
            setor TEXT,
            subsetor TEXT,
            min_52sem TEXT,
            max_52sem TEXT,
            vol_medio TEXT,
            val_mercado TEXT,
            val_firma TEXT,
            nro_acoes TEXT,
            p_l TEXT,
            p_vp TEXT,
            psr TEXT,
            div_yield TEXT,
            roe TEXT,
            roic TEXT,
            marg_bruta TEXT,
            marg_ebit TEXT,
            marg_liquida TEXT,
            ev_ebitda TEXT,
            lpa TEXT,
            vpa TEXT,
            ativo TEXT,
            disponibilidades TEXT,
            div_bruta TEXT,
            div_liquida TEXT,
            patrim_liquido TEXT,
            rec_liq_12m TEXT,
            lucro_liq_12m TEXT,
            cres_rec_5a TEXT
        );
        COMMIT;",
    )?;
    let has_revenue_growth = {
        let mut statement = conn.prepare("PRAGMA table_info(stock_details)")?;
        let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
        columns
            .collect::<Result<Vec<_>, _>>()?
            .iter()
            .any(|column| column == "cres_rec_5a")
    };
    if !has_revenue_growth {
        conn.execute("ALTER TABLE stock_details ADD COLUMN cres_rec_5a TEXT", [])?;
    }
    Ok(())
}

impl Stocks {
    pub fn write_stock(conn: &Connection, stock: &StockDetails) -> rusqlite::Result<i64> {
        conn.execute(
            "INSERT OR REPLACE INTO stock_details(ticker, nome, cotacao, data_cot, tipo, setor, subsetor, min_52sem, max_52sem, vol_medio, val_mercado, val_firma, nro_acoes, p_l, p_vp, psr, div_yield, roe, roic, marg_bruta, marg_ebit, marg_liquida, ev_ebitda, lpa, vpa, ativo, disponibilidades, div_bruta, div_liquida, patrim_liquido, rec_liq_12m, lucro_liq_12m, cres_rec_5a) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33)",
            rusqlite::params![
                &stock.ticker,
                &stock.nome,
                &stock.cotacao,
                &stock.data_cot,
                &stock.tipo,
                &stock.setor,
                &stock.subsetor,
                &stock.min_52sem,
                &stock.max_52sem,
                &stock.vol_medio,
                &stock.val_mercado,
                &stock.val_firma,
                &stock.nro_acoes,
                &stock.p_l,
                &stock.p_vp,
                &stock.psr,
                &stock.div_yield,
                &stock.roe,
                &stock.roic,
                &stock.marg_bruta,
                &stock.marg_ebit,
                &stock.marg_liquida,
                &stock.ev_ebitda,
                &stock.lpa,
                &stock.vpa,
                &stock.ativo,
                &stock.disponibilidades,
                &stock.div_bruta,
                &stock.div_liquida,
                &stock.patrim_liquido,
                &stock.rec_liq_12m,
                &stock.lucro_liq_12m,
                &stock.cres_rec_5a,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn write_stock_to_db(stock: &StockDetails) -> rusqlite::Result<i64> {
        let db = DB
            .get()
            .ok_or_else(|| rusqlite::Error::InvalidQuery)?
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        Stocks::write_stock(&db.conn, stock)
    }

    pub fn write(conn: &mut Connection, stocks: &[StockDetails]) -> rusqlite::Result<()> {
        let tx = conn.transaction()?;

        for stock in stocks {
            Stocks::write_stock(&tx, stock)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn read_all() -> rusqlite::Result<Vec<StockDetails>> {
        let db = DB
            .get()
            .ok_or_else(|| rusqlite::Error::InvalidQuery)?
            .lock()
            .unwrap();
        Self::read_all_from(&db.conn)
    }

    pub fn read_all_from(conn: &Connection) -> rusqlite::Result<Vec<StockDetails>> {
        let mut stmt = conn.prepare("SELECT * FROM stock_details")?;
        let stock_iter = stmt.query_map([], |row| {
            Ok(StockDetails {
                ticker: row.get(1)?,
                nome: row.get(2)?,
                cotacao: row.get(3)?,
                data_cot: row.get(4)?,
                tipo: row.get(5)?,
                setor: row.get(6)?,
                subsetor: row.get(7)?,
                min_52sem: row.get(8)?,
                max_52sem: row.get(9)?,
                vol_medio: row.get(10)?,
                val_mercado: row.get(11)?,
                val_firma: row.get(12)?,
                nro_acoes: row.get(13)?,
                p_l: row.get(14)?,
                p_vp: row.get(15)?,
                psr: row.get(16)?,
                div_yield: row.get(17)?,
                roe: row.get(18)?,
                roic: row.get(19)?,
                marg_bruta: row.get(20)?,
                marg_ebit: row.get(21)?,
                marg_liquida: row.get(22)?,
                ev_ebitda: row.get(23)?,
                lpa: row.get(24)?,
                vpa: row.get(25)?,
                ativo: row.get(26)?,
                disponibilidades: row.get(27)?,
                div_bruta: row.get(28)?,
                div_liquida: row.get(29)?,
                patrim_liquido: row.get(30)?,
                rec_liq_12m: row.get(31)?,
                lucro_liq_12m: row.get(32)?,
                cres_rec_5a: row
                    .get::<_, Option<String>>(33)?
                    .unwrap_or_else(|| "-".to_string()),
            })
        })?;

        let mut stocks = Vec::new();
        for stock in stock_iter {
            stocks.push(stock?);
        }
        Ok(stocks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initializes_and_round_trips_five_year_revenue_growth() {
        let connection = Connection::open_in_memory().unwrap();
        init(&connection).unwrap();
        init(&connection).unwrap();
        let stock = StockDetails {
            ticker: "TEST3".to_string(),
            cres_rec_5a: "12,5%".to_string(),
            ..StockDetails::default()
        };

        Stocks::write_stock(&connection, &stock).unwrap();
        let loaded = Stocks::read_all_from(&connection).unwrap();

        assert_eq!(loaded[0].cres_rec_5a, "12,5%");
    }

    #[test]
    fn adds_growth_column_to_an_existing_stock_details_table() {
        let connection = Connection::open_in_memory().unwrap();
        init(&connection).unwrap();
        Stocks::write_stock(
            &connection,
            &StockDetails {
                ticker: "LEGACY".to_string(),
                ..StockDetails::default()
            },
        )
        .unwrap();
        connection
            .execute("ALTER TABLE stock_details DROP COLUMN cres_rec_5a", [])
            .unwrap();
        init(&connection).unwrap();
        let loaded = Stocks::read_all_from(&connection).unwrap();

        assert_eq!(loaded[0].cres_rec_5a, "-");
    }
}
