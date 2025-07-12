#[macro_export]
macro_rules! INFO {
    ( $($args:expr),* ) => {
        slog_info!(slog_scope::logger(),$($args),*);
    };
}

#[macro_export]
macro_rules! ERROR {
    ( $($args:expr),* ) => {
        slog_error!(slog_scope::logger(),$($args),*);
    };
}

#[macro_export]
macro_rules! TRACE {
    ( $($args:expr),* ) => {
        slog_trace!(slog_scope::logger(),$($args),*);
    };
}

pub fn compute_change_pct(start: f64, end: f64) -> f64 {
    let frac = end / start;
    (frac - 1.0) * 100.0
}
