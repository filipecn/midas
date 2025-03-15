use ratatui::style::Color;

#[derive(Default)]
pub struct Curve {
    pub points: Vec<(f64, f64)>,
    pub origin: (f64, f64),
    pub data_bounds: [[f64; 2]; 2],
    pub color: Color,
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
