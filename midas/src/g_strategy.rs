use crate::g_indicators::IndicatorsGraph;
use dionysus::{finance::Sample, oracles::Advice, oracles::Oracle};

#[derive(Default)]
pub struct StrategyGraph {
    pub indicators: IndicatorsGraph,
    pub oracle: Oracle,
    pub advices: Vec<Advice>,
}

impl StrategyGraph {
    pub fn set_oracle(&mut self, oracle: &Oracle) {
        self.oracle = oracle.clone();
        for i in oracle.indicators() {
            self.indicators.add_indicator(&i);
        }
    }

    pub fn compute(&mut self, samples: &[Sample]) {
        self.indicators.compute(samples);
        self.advices = self.oracle.run_series(samples).unwrap();
    }
}
