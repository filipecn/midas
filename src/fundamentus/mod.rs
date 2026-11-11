pub mod db;
pub mod fetcher;
pub mod stock_view;
pub mod stocks_list;
pub mod view;

#[derive(Debug, Clone, Default)]
pub struct StockDetails {
    pub ticker: String,
    pub nome: String,
    pub cotacao: String,
    pub data_cot: String,
    pub tipo: String,
    pub setor: String,
    pub subsetor: String,
    pub min_52sem: String,
    pub max_52sem: String,
    pub vol_medio: String,
    pub val_mercado: String,
    pub val_firma: String,
    pub nro_acoes: String,

    // Valuation & Indicators
    pub p_l: String,
    pub p_vp: String,
    pub psr: String,
    pub div_yield: String,
    pub roe: String,
    pub roic: String,
    pub marg_bruta: String,
    pub marg_ebit: String,
    pub marg_liquida: String,
    pub ev_ebitda: String,
    pub lpa: String,
    pub vpa: String,

    // Balance Sheet & Income
    pub ativo: String,
    pub disponibilidades: String,
    pub div_bruta: String,
    pub div_liquida: String,
    pub patrim_liquido: String,
    pub rec_liq_12m: String,
    pub lucro_liq_12m: String,
    pub cres_rec_5a: String,
}

#[derive(Debug, Clone)]
pub struct StockSummary {
    pub ticker: String,
    pub cotacao: String,
    pub p_l: String,
    pub p_vp: String,
    pub div_yield: String,
    pub roe: String,
    pub qarp_score: Option<f64>,
    pub qarp_rank: Option<usize>,
    pub qarp_eligible: Option<bool>,
}
