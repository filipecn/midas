use crate::finance::{DiError, Sample};
use ta::indicators::{
    ExponentialMovingAverage, RelativeStrengthIndex, SimpleMovingAverage, StandardDeviation,
};
use ta::Next;

pub enum Indicator {
    ExponentialMovingAverage(u32),
    SimpleMovingAverage(u32),
    StandardDeviation(u32),
    RelativeStrengthIndex(u32),
}

impl Default for Indicator {
    fn default() -> Self {
        Indicator::SimpleMovingAverage(0 as u32)
    }
}

pub enum IndicatorData {
    Scalar(f64),
    Vector(Vec<f64>),
}

macro_rules! indicator_fn {
    ($name:tt, $func:ident) => {
        fn $name(n: usize, samples: &[Sample]) -> Result<IndicatorData, DiError> {
            let mut v: Vec<f64> = Vec::new();
            match $func::new(n) {
                Ok(mut f) => {
                    for sample in samples[n..].iter() {
                        let value = f.next(sample);
                        v.push(value);
                    }
                }
                Err(_) => (),
            }
            Ok(IndicatorData::Vector(v))
        }
    };
}

indicator_fn!(exponential_moving_average, ExponentialMovingAverage);
indicator_fn!(simple_moving_average, SimpleMovingAverage);
indicator_fn!(standard_deviation, StandardDeviation);
indicator_fn!(relative_strength_index, RelativeStrengthIndex);

macro_rules! match_indicator {
    ($func:ident, $words:expr) => {
        if $words.len() == 2 {
            match $words[1].parse::<u32>() {
                Ok(n) => return Some(Indicator::$func(n)),
                Err(_) => (),
            }
        }
    };
}

pub fn match_indicator_from_text(words: &[&str]) -> Option<Indicator> {
    match words[0].to_uppercase().as_str() {
        "RSI" => {
            match_indicator!(RelativeStrengthIndex, words)
        }
        "EMA" => {
            match_indicator!(ExponentialMovingAverage, words)
        }
        "SMA" => {
            match_indicator!(SimpleMovingAverage, words)
        }
        "SDEV" => {
            match_indicator!(StandardDeviation, words)
        }
        _ => (),
    };
    None
}

impl Indicator {
    pub fn compute(&self, samples: &[Sample]) -> Result<IndicatorData, DiError> {
        match &self {
            Self::ExponentialMovingAverage(n) => exponential_moving_average(*n as usize, samples),
            Self::SimpleMovingAverage(n) => simple_moving_average(*n as usize, samples),
            Self::StandardDeviation(n) => standard_deviation(*n as usize, samples),
            Self::RelativeStrengthIndex(n) => relative_strength_index(*n as usize, samples),
        }
    }
    pub fn to_string(&self) -> String {
        match &self {
            Self::ExponentialMovingAverage(n) => format!("EMA {:?}", n),
            Self::SimpleMovingAverage(n) => format!("SMA {:?}", n),
            Self::StandardDeviation(n) => format!("sdev {:?}", n),
            Self::RelativeStrengthIndex(n) => format!("rsi {:?}", n),
        }
    }
}
