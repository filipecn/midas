use std::cmp::Ordering;

#[macro_export]
macro_rules! INFO {
    ( $($args:tt),* ) => {
        slog_info!(slog_scope::logger(),$($args),*);
    };
}

#[macro_export]
macro_rules! ERROR {
    ( $f:tt, $($args:ident),* ) => {
        slog_error!(slog_scope::logger(),$f,$($args),*);
    };
}

#[macro_export]
macro_rules! TRACE {
    ( $($args:tt),* ) => {
        slog_trace!(slog_scope::logger(),$($args),*);
    };
}

pub fn compute_change_pct(start: f64, end: f64) -> f64 {
    if start.total_cmp(&end) == Ordering::Greater {
        start / end
    } else {
        -end / start
    }
}
