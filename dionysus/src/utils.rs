#[macro_export]
macro_rules! INFO {
    ( $($args:tt),* ) => {
        slog_info!(slog_scope::logger(),$($args),*);
    };
}

#[macro_export]
macro_rules! ERROR {
    ( $($args:tt),* ) => {
        slog_error!(slog_scope::logger(),$($args),*);
    };
}

#[macro_export]
macro_rules! TRACE {
    ( $($args:tt),* ) => {
        slog_trace!(slog_scope::logger(),$($args),*);
    };
}
