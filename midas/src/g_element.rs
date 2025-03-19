use std::f64;

use crate::{
    common::{color_from_signal, LOSS_COLOR, PROFIT_COLOR},
    g_common::ChartDomain,
    g_curve::Curve,
    g_indicators::{IndicatorGraph, IndicatorsGraph},
    g_samples::SamplesGraph,
    g_strategy::StrategyGraph,
};
use dionysus::{indicators::IndicatorSource, oracles::Signal};
use ratatui::{
    style::Styled,
    widgets::canvas::{Circle, Context, Line, Rectangle},
};

pub trait GraphElement {
    fn bounds(&self) -> [[f64; 2]; 2] {
        [[0.0, 0.0], [1.0, 1.0]]
    }
    fn draw(&self, domain: &ChartDomain, dest: &IndicatorSource, ctx: &mut Context);
}

impl GraphElement for Curve {
    fn bounds(&self) -> [[f64; 2]; 2] {
        self.data_bounds
    }

    fn draw(&self, domain: &ChartDomain, _dest: &IndicatorSource, ctx: &mut Context) {
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

    fn draw(&self, domain: &ChartDomain, dest: &IndicatorSource, ctx: &mut Context) {
        if *dest == IndicatorSource::Candle {
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
}

impl GraphElement for IndicatorGraph {
    fn draw(&self, domain: &ChartDomain, dest: &IndicatorSource, ctx: &mut Context) {
        match self {
            IndicatorGraph::SingleCurve(c) => c.draw(domain, dest, ctx),
            IndicatorGraph::Curves(m) => {
                for c in m {
                    c.draw(domain, dest, ctx);
                }
            }
            _ => (),
        }
    }
}

impl GraphElement for IndicatorsGraph {
    fn draw(&self, domain: &ChartDomain, dest: &IndicatorSource, ctx: &mut Context) {
        for (i, ig) in &self.indicators {
            if i.source() == *dest {
                ig.draw(domain, dest, ctx);
            }
        }
    }
}

impl GraphElement for StrategyGraph {
    fn draw(&self, domain: &ChartDomain, dest: &IndicatorSource, ctx: &mut Context) {
        self.indicators.draw(domain, dest, ctx);
        if *dest == IndicatorSource::Candle {
            //let mut line_points: Vec<(f64, f64)> = Vec::new();
            for (i, advice) in self.advices.iter().enumerate() {
                if advice.signal != Signal::None {
                    //line_points.push(((i / 2) as f64 * domain.dx, advice.stop_price));
                    //ctx.draw(&Circle {
                    //    x: (i / 2) as f64 * domain.dx,
                    //    y: advice.stop_price,
                    //    radius: 0.1,
                    //    color: color_from_signal(&advice.signal),
                    //});
                    ctx.print(
                        (i / 2) as f64 * domain.dx,
                        advice.stop_price,
                        format!("{:?}", advice.signal).set_style(color_from_signal(&advice.signal)),
                    );
                }
            }
            //for i in 1..line_points.len() {
            //    ctx.draw(&Line {
            //        x1: line_points[i - 1].0,
            //        x2: line_points[i].0,
            //        y1: line_points[i - 1].1,
            //        y2: line_points[i].1,
            //        color: color_from_signal(&Signal::None),
            //    });
            //}
        }
    }
}
