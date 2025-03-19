use ratatui::{
    style::{Color, Styled},
    widgets::canvas::Context,
};

#[derive(Default)]
pub struct ChartDomain {
    pub bounds: [[f64; 2]; 2],
    pub dx: f64,
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
        if step as usize == 0 {
            return;
        }
        for i in (bottom_price..top_price).step_by(step as usize) {
            ctx.print(
                x_offset,
                i as f64 / 100.0,
                format!("{:.2}", i as f64 / 100.0).set_style(Color::White),
            );
        }
    }
}
