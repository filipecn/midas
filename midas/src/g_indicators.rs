use crate::{g_common::ChartDomain, g_curve::Curve, g_element::GraphElement};
use dionysus::{
    finance::Sample,
    indicators::{Indicator, IndicatorData, IndicatorDomain, IndicatorSource},
};
use random_color::RandomColor;
use ratatui::{style::Color, widgets::canvas::Context};

pub enum IndicatorGraph {
    SingleCurve(Curve),
    Curves(Vec<Curve>),
}

impl IndicatorGraph {
    pub fn get_color(&self) -> Color {
        match self {
            IndicatorGraph::SingleCurve(c) => c.color,
            IndicatorGraph::Curves(m) => m[0].color,
        }
    }
}

#[derive(Default)]
pub struct IndicatorsGraph {
    pub indicators: Vec<(Indicator, IndicatorGraph)>,
}

impl IndicatorsGraph {
    fn curve_from_scalar(&self, x_end: f64, s: f64, y0: f64) -> Curve {
        let mut points = Vec::new();
        points.push((0.0, s));
        points.push((x_end, s));
        let mut c = Curve::default();
        c.points = points;
        c.origin = (0.0, y0);
        c.compute_bounds();
        c
    }

    fn curve_from_vector(&self, x: usize, v: &Vec<f64>, color: &Color, y0: f64) -> Curve {
        let mut i = x;
        let mut points = Vec::new();
        for s in v {
            points.push((i as f64, *s));
            i += 1;
        }
        let mut c = Curve::default();
        c.origin = (0.0, y0);
        c.color = color.clone();
        c.points = points;
        c.compute_bounds();
        c
    }

    fn curves_from_matrix(
        &self,
        x: usize,
        v: &Vec<Vec<f64>>,
        color: &Color,
        y0: f64,
    ) -> Vec<Curve> {
        let mut curves: Vec<Curve> = Vec::new();
        for i in 0..v.len() {
            let mut xi = x;
            let mut points = Vec::new();
            for j in 0..v[0].len() {
                points.push((xi as f64, v[i][j]));
                xi += 1;
            }
            let mut c = Curve::default();
            c.points = points;
            c.origin = (0.0, y0);
            c.color = color.clone();
            c.compute_bounds();
            curves.push(c);
        }
        curves
    }

    pub fn add_indicator(&mut self, indicator: &Indicator, samples: &[Sample]) {
        let mut rng_color = RandomColor::new();
        let rgb = rng_color.to_rgb_array();
        let color = Color::Rgb(rgb[0], rgb[1], rgb[2]);
        let mut y0 = 0.0;
        if indicator.source() == IndicatorSource::Candle {
            match indicator.domain() {
                IndicatorDomain::Price => y0 = 0.0,
                _ => y0 = samples.last().unwrap().open,
            }
        }
        match indicator.compute_series(samples) {
            Ok(r) => match r {
                IndicatorData::Scalar(s) => {
                    let curve = self.curve_from_scalar(samples.len() as f64, s, y0);
                    self.indicators
                        .push((indicator.clone(), IndicatorGraph::SingleCurve(curve)));
                }
                IndicatorData::Vector(v) => {
                    let curve = self.curve_from_vector(
                        samples.len().saturating_sub(v.len()),
                        &v,
                        &color,
                        y0,
                    );
                    self.indicators
                        .push((indicator.clone(), IndicatorGraph::SingleCurve(curve)));
                }
                IndicatorData::Matrix(m) => {
                    let curves = self.curves_from_matrix(
                        samples.len().saturating_sub(m[0].len()),
                        &m,
                        &color,
                        y0,
                    );
                    self.indicators
                        .push((indicator.clone(), IndicatorGraph::Curves(curves)));
                }
            },
            _ => (),
        };
    }

    pub fn compute(&mut self, samples: &[Sample]) {
        for i in 0..self.indicators.len() {
            let mut y0 = 0.0;
            if self.indicators[i].0.source() == IndicatorSource::Candle {
                match self.indicators[i].0.domain() {
                    IndicatorDomain::Price => y0 = 0.0,
                    _ => y0 = samples.last().unwrap().open,
                }
            }
            match self.indicators[i].0.compute_series(samples) {
                Ok(r) => match r {
                    IndicatorData::Scalar(s) => {
                        self.indicators[i].1 = IndicatorGraph::SingleCurve(self.curve_from_scalar(
                            samples.len() as f64,
                            s,
                            y0,
                        ));
                    }
                    IndicatorData::Vector(v) => {
                        self.indicators[i].1 = IndicatorGraph::SingleCurve(self.curve_from_vector(
                            samples.len().saturating_sub(v.len()),
                            &v,
                            &self.indicators[i].1.get_color(),
                            y0,
                        ))
                    }
                    IndicatorData::Matrix(m) => {
                        self.indicators[i].1 = IndicatorGraph::Curves(self.curves_from_matrix(
                            samples.len().saturating_sub(m[0].len()),
                            &m,
                            &self.indicators[i].1.get_color(),
                            y0,
                        ))
                    }
                },
                _ => (),
            };
        }
    }

    pub fn draw_volume(&self, domain: &ChartDomain, ctx: &mut Context) {
        for (i, ig) in &self.indicators {
            if i.source() == IndicatorSource::Volume {
                ig.draw(domain, ctx);
            }
        }
    }
}
