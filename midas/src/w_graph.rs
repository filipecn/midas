use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Styled,
    symbols::{self},
    text::Line,
    widgets::{canvas::Canvas, Block, Borders, Paragraph, Widget},
};

use crate::{
    common, g_book::BookGraph, g_common::ChartDomain, g_element::GraphElement,
    g_samples::SamplesGraph, g_strategy::StrategyGraph,
};
use dionysus::{
    backtest::Backtest,
    finance::Sample,
    indicators::{Indicator, IndicatorSource},
    time::TimeWindow,
};

pub struct StockGraph {
    pub book_w: BookGraph,
    pub candle_w: ChartDomain,
    pub volume_w: ChartDomain,
    pub zooming: bool,
    pub focus: bool,
    pub samples: SamplesGraph,
    pub strategies: Vec<(String, StrategyGraph)>,
    pub selected_strategy: usize,
    pub time_window: TimeWindow,
}

impl Default for StockGraph {
    fn default() -> Self {
        Self {
            book_w: BookGraph::default(),
            candle_w: ChartDomain::default(),
            volume_w: ChartDomain::default(),
            zooming: false,
            focus: false,
            samples: SamplesGraph::default(),
            strategies: vec![(String::from("CHART"), StrategyGraph::default())],
            selected_strategy: 0,
            time_window: TimeWindow::default(),
        }
    }
}

impl StockGraph {
    pub fn set_data(&mut self, samples: &[Sample]) {
        self.samples.update(samples);
        self.time_window.resolution = samples[0].resolution.clone();
        self.time_window.count = samples.len() as i64;
        self.strategies[self.selected_strategy].1.compute(samples);
        self.book_w.x_pos = samples.len() as f64;
        self.candle_w.timestamp = samples[0].timestamp;
        self.candle_w.time_step = samples[0].resolution.num_seconds() as u64 * 1000;
    }

    pub fn add_indicator(&mut self, indicator: &Indicator) {
        self.strategies[0].1.indicators.add_indicator(indicator);
    }

    pub fn set_backtest(&mut self, backtest: &Backtest) {
        self.strategies[self.selected_strategy]
            .1
            .set_backtest(backtest);
    }

    pub fn reset_camera(&mut self) {
        self.update_bounds();
    }

    fn update_bounds(&mut self) {
        self.candle_w.dx = 2.0;
        self.candle_w.bounds = self.samples.bounds();
        self.candle_w.bounds[0][0] *= self.candle_w.dx;
        self.candle_w.bounds[0][1] *= self.candle_w.dx;

        self.volume_w.dx = 2.0;
        self.volume_w.bounds = self.samples.bounds();
        self.volume_w.bounds[0][0] *= self.volume_w.dx;
        self.volume_w.bounds[0][1] *= self.volume_w.dx;
        self.volume_w.bounds[1][0] = 0.0;
        self.volume_w.bounds[1][1] = 100.0;
    }

    pub fn pan(&mut self, dx: f64, dy: f64) {
        let x = (self.candle_w.bounds[0][1] - self.candle_w.bounds[0][0]) * 0.05 * dx;
        let y = (self.candle_w.bounds[1][1] - self.candle_w.bounds[1][0]) * 0.05 * dy;
        self.candle_w.bounds[0][0] += x;
        self.candle_w.bounds[0][1] += x;
        self.candle_w.bounds[1][0] += y;
        self.candle_w.bounds[1][1] += y;

        self.volume_w.bounds[0][0] += x;
        self.volume_w.bounds[0][1] += x;
    }

    pub fn zoom(&mut self, dx: f64, dy: f64) {
        let x_zoom = (self.candle_w.bounds[0][1] - self.candle_w.bounds[0][0]) * dx;
        let y_zoom = (self.candle_w.bounds[1][1] - self.candle_w.bounds[1][0]) * dy;
        self.candle_w.bounds[0][0] += x_zoom;
        self.candle_w.bounds[0][1] -= x_zoom;
        self.candle_w.bounds[1][0] += y_zoom;
        self.candle_w.bounds[1][1] -= y_zoom;

        self.volume_w.bounds[0][0] += x_zoom;
        self.volume_w.bounds[0][1] -= x_zoom;
    }

    pub fn draw_legend(&self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line> = Vec::new();
        for (_, (indicator, ig)) in self.strategies[self.selected_strategy]
            .1
            .indicators
            .indicators
            .iter()
            .enumerate()
        {
            lines.push(Line::from(indicator.to_string()).set_style(ig.get_color()));
        }

        Paragraph::new(lines)
            .block(Block::bordered().title(format!(
                "Indicators ({:?})",
                self.strategies[self.selected_strategy].0
            )))
            .render(area, buf);
    }
}

impl Widget for &StockGraph {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::vertical([Constraint::Percentage(80), Constraint::Percentage(20)]);
        let [candle_area, volume_area] = layout.areas(area);
        let mut title: String = String::from("Chart ");
        title.push_str(self.candle_w.sample_count().to_string().as_str());
        title.push_str("@");
        title.push_str(self.samples.data[0].resolution.name().as_str());
        Canvas::default()
            .block(
                common::block(title.as_str())
                    .style(common::focus_style(self.focus))
                    .borders(Borders::LEFT | Borders::RIGHT | Borders::TOP),
            )
            .marker(symbols::Marker::Braille)
            .x_bounds(self.candle_w.bounds[0])
            .y_bounds(self.candle_w.bounds[1])
            .paint(|ctx| {
                self.samples
                    .draw(&self.candle_w, &IndicatorSource::Candle, ctx);
                self.strategies[self.selected_strategy].1.draw(
                    &self.candle_w,
                    &IndicatorSource::Candle,
                    ctx,
                );
                self.candle_w.draw(ctx);
                self.book_w
                    .draw(&self.candle_w, &IndicatorSource::Candle, ctx);
            })
            .render(candle_area, buf);
        Canvas::default()
            .block(
                common::block("")
                    .style(common::focus_style(self.focus))
                    .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM),
            )
            .marker(symbols::Marker::Braille)
            .x_bounds(self.volume_w.bounds[0])
            .y_bounds(self.volume_w.bounds[1])
            .paint(|ctx| {
                self.samples.draw_volume(&self.volume_w, ctx);
                self.strategies[self.selected_strategy].1.draw(
                    &self.volume_w,
                    &IndicatorSource::Volume,
                    ctx,
                );
                self.volume_w.draw(ctx);
            })
            .render(volume_area, buf);
    }
}
