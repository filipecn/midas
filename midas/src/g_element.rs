use crate::{
    common::{LOSS_COLOR, PROFIT_COLOR},
    g_common::ChartDomain,
    g_curve::Curve,
    g_indicators::{IndicatorGraph, IndicatorsGraph},
    g_samples::SamplesGraph,
};
use dionysus::indicators::IndicatorSource;
use ratatui::widgets::canvas::{Context, Line, Rectangle};

pub trait GraphElement {
    fn bounds(&self) -> [[f64; 2]; 2];
    fn draw(&self, domain: &ChartDomain, ctx: &mut Context);
}

impl GraphElement for Curve {
    fn bounds(&self) -> [[f64; 2]; 2] {
        self.data_bounds
    }
    fn draw(&self, domain: &ChartDomain, ctx: &mut Context) {
        for i in 1..self.points.len() {
            ctx.draw(&Line::new(
                self.points[i - 1].0 * domain.dx + self.origin.0,
                self.points[i - 1].1 + self.origin.1,
                self.points[i].0 * domain.dx + self.origin.0,
                self.points[i].1 + self.origin.1,
                self.color,
            ));
        }
    }
}

impl GraphElement for SamplesGraph {
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

            ctx.draw(&Line::new(
                x,
                sample.low,
                x,
                sample.close.min(sample.open),
                candle_color,
            ));

            ctx.draw(&Line::new(
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
    }
}

impl GraphElement for IndicatorGraph {
    fn bounds(&self) -> [[f64; 2]; 2] {
        [[0.0, 0.0], [1.0, 1.0]]
    }

    fn draw(&self, domain: &ChartDomain, ctx: &mut Context) {
        match self {
            IndicatorGraph::SingleCurve(c) => c.draw(domain, ctx),
            IndicatorGraph::Curves(m) => {
                for c in m {
                    c.draw(domain, ctx);
                }
            }
        }
    }
}

impl GraphElement for IndicatorsGraph {
    fn bounds(&self) -> [[f64; 2]; 2] {
        [[0.0, 0.0], [1.0, 1.0]]
    }

    fn draw(&self, domain: &ChartDomain, ctx: &mut Context) {
        for (i, ig) in &self.indicators {
            if i.source() == IndicatorSource::Candle {
                ig.draw(domain, ctx);
            }
        }
    }
}
