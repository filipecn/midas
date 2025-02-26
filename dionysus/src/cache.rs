use crate::finance::{DiError, Sample};
use crate::time::{Period, TimeUnit, TimeWindow};
use std::collections::HashMap;

pub type SampleCache = HashMap<TimeUnit, Vec<Sample>>;
pub type SymbolCache = HashMap<String, SampleCache>;

#[derive(Default)]
pub struct Cache {
    data: SymbolCache,
}

impl Cache {
    pub fn contains(&self, symbol: &str, period: &Period) -> bool {
        if let Some(unit_cache) = self.data.get(symbol) {
            if let Some(cache) = unit_cache.get(&period.duration.resolution) {
                if cache.is_empty() {
                    return false;
                }
                if cache.first().unwrap().timestamp > period.end().timestamp() as u64 {
                    return false;
                }
                if cache.last().unwrap().timestamp < period.end().timestamp() as u64 {
                    return false;
                }
                return true;
            }
        }
        false
    }
    pub fn read(&self, symbol: &str, duration: &TimeWindow) -> Result<&[Sample], DiError> {
        match self
            .data
            .get(symbol)
            .and_then(|unit_cache| unit_cache.get(&duration.resolution))
        {
            Some(samples) => {
                let first_index = samples.len().saturating_sub(duration.count as usize);
                return Ok(&samples[first_index..]);
            }
            None => return Err(DiError::NotFound),
        }
    }
    pub fn write(&mut self, symbol: &str, samples: &[Sample]) -> Result<(), DiError> {
        let v: Vec<Sample> = samples.iter().map(|sample| sample.clone()).collect();
        if v.is_empty() {
            return Ok(());
        }
        let resolution = v[0].resolution.clone();
        match &mut self.data.get_mut(symbol) {
            Some(unit_cache) => match unit_cache.get_mut(&resolution) {
                Some(cache) => {
                    if v[0].timestamp <= cache[0].timestamp {
                        return Err(DiError::NotImplemented);
                    } else {
                        for sample in v {
                            cache.push(sample);
                        }
                    }
                }
                None => {
                    unit_cache.insert(resolution, v);
                }
            },
            None => {
                let mut sample_cache = SampleCache::new();
                sample_cache.insert(resolution, v);
                self.data.insert(symbol.to_string(), sample_cache);
            }
        }
        Ok(())
    }
}
