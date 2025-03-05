use crate::common;
use crate::common::{LOSS_COLOR, PROFIT_COLOR};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use random_color::RandomColor;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Styled},
    symbols::{self},
    text::Line,
    widgets::{
        canvas::{self, Canvas, Context, Rectangle},
        Block, Paragraph, Widget,
    },
};

use crate::common::Interactible;
use dionysus::{
    binance::BinanceMarket,
    brownian::BrownianMotionMarket,
    finance::{DiError, Sample, Token},
    historical_data::HistoricalData,
    ta::{Indicator, IndicatorData},
    time::{TimeUnit, TimeWindow},
};

fn get_provider(token: &Token) -> Box<dyn HistoricalData> {
    match token.get_symbol().as_str() {
        "brownian" => Box::new(BrownianMotionMarket::default()),
        &_ => Box::new(BinanceMarket::default()),
    }
}

pub trait GraphElement {
    fn bounds(&self) -> [[f64; 2]; 2];
    fn draw(&self, domain: &ChartDomain, ctx: &mut Context);
}

#[derive(Default)]
pub struct ChartDomain {
    bounds: [[f64; 2]; 2],
    dx: f64,
}

impl ChartDomain {
    pub fn size(&self, dim: usize) -> f64 {
        self.bounds[dim][1] - self.bounds[dim][0]
    }

    pub fn sample_count(&self) -> u64 {
        (self.size(0) / self.dx) as u64
    }

    pub fn draw(&self, ctx: &mut Context) {
        let x_offset = self.bounds[0][0] + self.size(0) * 0.01;
        let bottom_price = (self.bounds[1][0] * 100.0).floor() as i64;
        let top_price = (self.bounds[1][1] * 100.0).ceil() as i64;
        let step = (top_price - bottom_price) as f64 * 0.2;
        for i in (bottom_price..top_price).step_by(step as usize) {
            ctx.print(
                x_offset,
                i as f64 / 100.0,
                format!("{:.2}", i as f64 / 100.0).set_style(Color::White),
            );
        }
    }
}

#[derive(Default)]
pub struct Curve {
    pub points: Vec<(f64, f64)>,
    data_bounds: [[f64; 2]; 2],
    color: Color,
}

impl Curve {
    pub fn compute_bounds(&mut self) {
        let mut price_bounds = [self.points[0].1, self.points[0].1];
        let mut time_bounds = [self.points[0].0, self.points[0].0];

        for point in &self.points {
            price_bounds[0] = (price_bounds[0] as f64).min(point.1);
            price_bounds[1] = (price_bounds[1] as f64).max(point.1);
            time_bounds[0] = (time_bounds[0] as f64).min(point.0);
            time_bounds[1] = (time_bounds[1] as f64).max(point.0);
        }

        self.data_bounds[0] = time_bounds;
        self.data_bounds[1] = price_bounds;
    }
}

impl GraphElement for Curve {
    fn bounds(&self) -> [[f64; 2]; 2] {
        self.data_bounds
    }
    fn draw(&self, domain: &ChartDomain, ctx: &mut Context) {
        for i in 1..self.points.len() {
            ctx.draw(&canvas::Line::new(
                self.points[i - 1].0 * domain.dx,
                self.points[i - 1].1,
                self.points[i].0 * domain.dx,
                self.points[i].1,
                self.color,
            ));
        }
    }
}

#[derive(Default)]
pub struct Samples {
    pub token: Token,
    pub data: Vec<Sample>,
    data_bounds: [[f64; 2]; 2],
    indicators: Vec<(Indicator, Curve)>,
}

impl GraphElement for Samples {
    fn bounds(&self) -> [[f64; 2]; 2] {
        self.data_bounds
    }

    fn draw(&self, domain: &ChartDomain, ctx: &mut Context) {
        // candlestick
        let mut i = 0;
        for sample in &self.data {
            let candle_color = if sample.close > sample.open {
                PROFIT_COLOR
            } else {
                LOSS_COLOR
            };

            let x = domain.dx * i as f64;

            ctx.draw(&canvas::Line::new(
                x,
                sample.low,
                x,
                sample.close.min(sample.open),
                candle_color,
            ));

            ctx.draw(&canvas::Line::new(
                x,
                sample.high,
                x,
                sample.close.max(sample.open),
                candle_color,
            ));

            ctx.draw(&Rectangle {
                x: x - 0.3,
                y: if sample.close > sample.open {
                    sample.open
                } else {
                    sample.close
                },
                width: 0.6,
                height: (sample.close - sample.open).abs(),
                color: candle_color,
            });
            i += 1;
        }
        // indicators
        for (_, curve) in &self.indicators {
            curve.draw(domain, ctx);
        }
    }
}

impl Samples {
    pub fn load(&mut self, token: &Token, time_window: &TimeWindow) -> Result<(), DiError> {
        let mut provider = get_provider(token);
        match provider.get_last(token.to_string().to_uppercase().as_str(), &time_window) {
            Ok(samples) => {
                self.token = token.clone();
                self.data.clear();
                for sample in samples {
                    self.data.push(sample.clone());
                }
                self.compute_bounds();
                self.compute_indicators();
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    pub fn add_indicator(&mut self, indicator: Indicator) {
        let mut rng_color = RandomColor::new();
        let rgb = rng_color.to_rgb_array();
        let color = Color::Rgb(rgb[0], rgb[1], rgb[2]);
        match indicator.compute(&self.data[..]) {
            Ok(r) => match r {
                IndicatorData::Scalar(s) => {
                    let curve = self.curve_from_scalar(s);
                    self.indicators.push((indicator, curve));
                }
                IndicatorData::Vector(v) => {
                    let curve = self.curve_from_vector(&v, &color);
                    self.indicators.push((indicator, curve));
                }
            },
            _ => (),
        };
    }

    pub fn compute_indicators(&mut self) {
        for i in 0..self.indicators.len() {
            match self.indicators[i].0.compute(&self.data[..]) {
                Ok(r) => match r {
                    IndicatorData::Scalar(s) => {
                        self.indicators[i].1 = self.curve_from_scalar(s);
                    }
                    IndicatorData::Vector(v) => {
                        self.indicators[i].1 =
                            self.curve_from_vector(&v, &self.indicators[i].1.color);
                    }
                },
                _ => (),
            };
        }
    }

    fn curve_from_scalar(&self, s: f64) -> Curve {
        let mut points = Vec::new();
        points.push((0.0, s));
        points.push((self.data.len() as f64, s));
        let mut c = Curve::default();
        c.points = points;
        c.compute_bounds();
        c
    }

    fn curve_from_vector(&self, v: &Vec<f64>, color: &Color) -> Curve {
        let mut i = self.data.len().saturating_sub(v.len());
        let mut points = Vec::new();
        for s in v {
            points.push((i as f64, *s));
            i += 1;
        }
        let mut c = Curve::default();
        c.color = color.clone();
        c.points = points;
        c.compute_bounds();
        c
    }

    fn compute_bounds(&mut self) {
        let mut price_bounds = [self.data[0].low, self.data[0].high];
        let time_bounds = [0.0, self.data.len() as f64];

        for sample in &self.data {
            price_bounds[0] = (price_bounds[0] as f64).min(sample.low);
            price_bounds[1] = (price_bounds[1] as f64).max(sample.high);
        }

        self.data_bounds[0] = time_bounds;
        self.data_bounds[1] = price_bounds;
    }
}

#[derive(Default)]
pub struct StockGraph {
    samples: Samples,
    window: ChartDomain,
    zooming: bool,
    focus: bool,
}

impl StockGraph {
    pub fn update_with(&mut self, token: &Token, sample: &Sample) {
        if !self.samples.data.is_empty()
            && self.samples.data.last().unwrap().resolution != sample.resolution
        {
            return;
        }
        if *token == self.samples.token {
            if self.samples.data.is_empty()
                || self.samples.data.last().unwrap().timestamp != sample.timestamp
            {
                self.samples.data.push(sample.clone());
            } else {
                let last_i = self.samples.data.len() - 1;
                self.samples.data[last_i] = sample.clone();
            }
            self.samples.compute_indicators();
        }
    }

    pub fn add_indicator(&mut self, indicator: Indicator) {
        self.samples.add_indicator(indicator);
    }

    fn update_bounds(&mut self) {
        self.window.dx = 2.0;
        self.window.bounds = self.samples.bounds();
        self.window.bounds[0][0] *= self.window.dx;
        self.window.bounds[0][1] *= self.window.dx;
    }

    pub fn load(&mut self, token: &Token, resolution: &TimeUnit, n: usize) -> Result<(), DiError> {
        self.samples.load(
            token,
            &TimeWindow {
                resolution: resolution.clone(),
                count: n as i64,
            },
        )?;
        self.update_bounds();
        Ok(())
    }

    pub fn set_resolution(&mut self, resolution: &TimeUnit) -> Result<(), DiError> {
        self.load(
            &self.samples.token.clone(),
            resolution,
            self.samples.data.len(),
        )?;
        self.update_bounds();
        Ok(())
    }

    pub fn pan(&mut self, dx: f64, dy: f64) {
        let x = (self.window.bounds[0][1] - self.window.bounds[0][0]) * 0.05 * dx;
        let y = (self.window.bounds[1][1] - self.window.bounds[1][0]) * 0.05 * dy;
        self.window.bounds[0][0] += x;
        self.window.bounds[0][1] += x;
        self.window.bounds[1][0] += y;
        self.window.bounds[1][1] += y;
    }

    pub fn zoom(&mut self, dx: f64, dy: f64) {
        let x_zoom = (self.window.bounds[0][1] - self.window.bounds[0][0]) * dx;
        let y_zoom = (self.window.bounds[1][1] - self.window.bounds[1][0]) * dy;
        self.window.bounds[0][0] += x_zoom;
        self.window.bounds[0][1] -= x_zoom;
        self.window.bounds[1][0] += y_zoom;
        self.window.bounds[1][1] -= y_zoom;
    }

    pub fn legend_area(&self, area: Rect) -> Option<Rect> {
        if !self.samples.indicators.is_empty() {
            let vertical = Layout::vertical([Constraint::Percentage(20)]).flex(Flex::Start);
            let horizontal = Layout::horizontal([Constraint::Percentage(10)]).flex(Flex::End);
            let [area] = vertical.areas(area);
            let [area] = horizontal.areas(area);
            Some(area)
        } else {
            None
        }
    }

    pub fn draw_legend(&self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line> = Vec::new();
        for (_, (indicator, curve)) in self.samples.indicators.iter().enumerate() {
            lines.push(Line::from(indicator.to_string()).set_style(curve.color));
        }

        Paragraph::new(lines)
            .block(Block::bordered().title("Indicators"))
            .render(area, buf);
    }
}

impl Interactible for StockGraph {
    fn handle_key_event(&mut self, key_event: &KeyEvent) -> bool {
        let mut consumed = true;
        if key_event.kind == KeyEventKind::Press {
            match key_event.modifiers {
                KeyModifiers::CONTROL => self.zooming = true,
                _ => self.zooming = false,
            };
            match key_event.code {
                KeyCode::Left => {
                    if self.zooming {
                        self.zoom(-0.05, 0.0);
                    } else {
                        self.pan(-1.0, 0.0);
                    }
                }
                KeyCode::Right => {
                    if self.zooming {
                        self.zoom(0.05, 0.0);
                    } else {
                        self.pan(1.0, 0.0);
                    }
                }
                KeyCode::Up => {
                    if self.zooming {
                        self.zoom(0.0, 0.05);
                    } else {
                        self.pan(0.0, 1.0);
                    }
                }
                KeyCode::Down => {
                    if self.zooming {
                        self.zoom(0.0, -0.05);
                    } else {
                        self.pan(0.0, -1.0);
                    }
                }
                _ => consumed = false,
            };
        }
        consumed
    }
    fn set_focus(&mut self, focus: bool) {
        self.focus = focus;
    }
}

impl Widget for &StockGraph {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut title: String = String::from("Chart ");
        title.push_str(self.window.sample_count().to_string().as_str());
        title.push_str("@");
        title.push_str(self.samples.data[0].resolution.name().as_str());
        Canvas::default()
            .block(common::block(title.as_str()).style(common::focus_style(self.focus)))
            .marker(symbols::Marker::Braille)
            .x_bounds(self.window.bounds[0])
            .y_bounds(self.window.bounds[1])
            .paint(|ctx| {
                self.samples.draw(&self.window, ctx);
                self.window.draw(ctx);
            })
            .render(area, buf);
    }
}
