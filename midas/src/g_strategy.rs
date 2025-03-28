use crate::g_indicators::IndicatorsGraph;
use dionysus::{counselor::Advice, finance::Sample, strategy::Strategy};

#[derive(Default)]
pub struct StrategyGraph {
    pub indicators: IndicatorsGraph,
    pub advices: Vec<Advice>,
}

impl StrategyGraph {
    pub fn set_strategy(&mut self, strategy: &Strategy) {
        for c in strategy.counselors.iter() {
            for i in c.indicators() {
                self.indicators.add_indicator(&i);
            }
        }
    }

    pub fn compute(&mut self, samples: &[Sample]) {
        self.indicators.compute(samples);
        //self.advices = self.oracle.run_series(samples).unwrap();
    }
}
