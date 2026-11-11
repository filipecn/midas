use std::cell::RefCell;

use ratatui::style::Color;

thread_local! {
    static STATIC_TUI_DATA: RefCell<TUI> = {
        RefCell::new(TUI::default())
    }
}

#[derive(Clone)]
pub struct ColorPalette {
    pub green: Color,
    pub budget: Color,
    pub spent: Color,
}

impl Default for ColorPalette {
    fn default() -> Self {
        Self {
            green: Color::Green,
            budget: Color::Rgb(80, 180, 220),
            spent: Color::Red,
        }
    }
}

pub struct TUI {
    pub color_palette: ColorPalette,
    pub show_sensitive_values: bool,
}

impl Default for TUI {
    fn default() -> Self {
        Self {
            color_palette: ColorPalette::default(),
            show_sensitive_values: true,
        }
    }
}

impl TUI {
    pub fn color_palette() -> ColorPalette {
        STATIC_TUI_DATA.with_borrow(|data| data.color_palette.clone())
    }

    pub fn sensitive_values_visible() -> bool {
        STATIC_TUI_DATA.with_borrow(|data| data.show_sensitive_values)
    }

    pub fn set_sensitive_values_visible(visible: bool) {
        STATIC_TUI_DATA.with_borrow_mut(|data| data.show_sensitive_values = visible);
    }

    pub fn format_amount_for_display(amount_in_cents: i64, width: usize) -> String {
        if STATIC_TUI_DATA.with_borrow(|data| data.show_sensitive_values) {
            format!(
                "{:>width$}",
                format!("{:.2}", amount_in_cents as f64 / 100.0),
                width = width
            )
        } else {
            let mask = "•".repeat(width.max(6).min(12));
            format!("{:>width$}", mask, width = width)
        }
    }
}
