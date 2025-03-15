use crate::{
    common::{LOSS_COLOR, PROFIT_COLOR},
    g_common::ChartDomain,
};
use dionysus::finance::Sample;
use ratatui::widgets::canvas::{Context, Rectangle};

#[derive(Default)]
pub struct SamplesGraph {
    pub data: Vec<Sample>,
    pub data_bounds: [[f64; 2]; 2],
}

impl SamplesGraph {
    pub fn update(&mut self, samples: &[Sample]) {
        self.data.clear();
        self.data = samples.iter().map(|x| x.clone()).collect();
        self.compute_bounds();
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

    pub fn draw_volume(&self, domain: &ChartDomain, ctx: &mut Context) {
        // candlestick
        let mut i = 0;

        let max_volume = self
            .data
            .iter()
            .max_by(|a, b| a.volume.cmp(&b.volume))
            .unwrap()
            .volume;

        let scale = 100.0 / (max_volume as f64);

        for sample in &self.data {
            let candle_color = if sample.close > sample.open {
                PROFIT_COLOR
            } else {
                LOSS_COLOR
            };

            let x = domain.dx * i as f64;

            ctx.draw(&Rectangle {
                x: x - 0.3,
                y: 0.0,
                width: 0.6,
                height: (sample.volume as f64) * scale,
                color: candle_color,
            });
            i += 1;
        }
    }
}
