use std::collections::BTreeMap;

use crate::{
    fundamentus::StockSummary,
    widgets::list::{GenericList, ListColumn},
};

pub fn create_stocks_summaries_list(summaries: Vec<StockSummary>) -> GenericList<StockSummary> {
    let columns = BTreeMap::from([
        (
            "Ticker".to_string(),
            ListColumn::new(10, |summary: &StockSummary| summary.ticker.clone()),
        ),
        (
            "Quote".to_string(),
            ListColumn::new(12, |summary: &StockSummary| summary.cotacao.clone()),
        ),
        (
            "P/L".to_string(),
            ListColumn::new(10, |summary: &StockSummary| summary.p_l.clone()),
        ),
        (
            "P/VP".to_string(),
            ListColumn::new(10, |summary: &StockSummary| summary.p_vp.clone()),
        ),
        (
            "Dividend Yield".to_string(),
            ListColumn::new(16, |summary: &StockSummary| summary.div_yield.clone()),
        ),
        (
            "ROE".to_string(),
            ListColumn::new(10, |summary: &StockSummary| summary.roe.clone()),
        ),
        (
            "QARP".to_string(),
            ListColumn::new(12, |summary: &StockSummary| {
                match (summary.qarp_eligible, summary.qarp_score) {
                    (Some(true), Some(score)) => summary.qarp_rank.map_or_else(
                        || format!("{score:.1}"),
                        |rank| format!("{rank}:{score:.1}"),
                    ),
                    (Some(false), _) => "Ineligible".to_string(),
                    _ => "-".to_string(),
                }
            }),
        ),
    ]);

    GenericList::new(summaries, columns)
}
