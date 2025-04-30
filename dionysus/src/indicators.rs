use crate::finance::{DiError, Sample, F64};
use ta::indicators::{
    BollingerBands, ExponentialMovingAverage, MovingAverageConvergenceDivergence,
    RelativeStrengthIndex, SimpleMovingAverage, StandardDeviation,
};
use ta::Next;

#[derive(PartialEq, Eq)]
pub enum IndicatorSource {
    Candle,
    Volume,
}

#[derive(PartialEq, Default, Eq)]
pub enum IndicatorDomain {
    Percent,
    Unit,
    Cartesian,
    #[default]
    Price,
    Volume,
}

#[derive(Default)]
pub struct Indicators {
    indicators: Vec<Indicator>,
}

#[derive(PartialEq, Eq, Clone)]
pub enum Indicator {
    ExponentialMovingAverage(usize),
    SimpleMovingAverage(usize),
    StandardDeviation(usize),
    RelativeStrengthIndex(usize),
    BollingerBands((usize, F64)),
    MovingAverageConvergenceDivergence((usize, usize, usize)),
    SupportLines(F64),
    ResistanceLines(F64),
}

impl Default for Indicator {
    fn default() -> Self {
        Indicator::SimpleMovingAverage(0 as usize)
    }
}

pub enum IndicatorData {
    Scalar(f64),
    Vector(Vec<f64>),
    Matrix(Vec<Vec<f64>>),
}

macro_rules! indicator_series_fn {
    ($name:tt, $func:ident) => {
        fn $name(n: usize, samples: &[Sample]) -> Result<IndicatorData, DiError> {
            let mut v: Vec<f64> = Vec::new();
            match $func::new(n) {
                Ok(mut f) => {
                    for sample in samples {
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

indicator_series_fn!(exponential_moving_average_s, ExponentialMovingAverage);
indicator_series_fn!(simple_moving_average_s, SimpleMovingAverage);
indicator_series_fn!(standard_deviation_s, StandardDeviation);
indicator_series_fn!(relative_strength_index_s, RelativeStrengthIndex);

macro_rules! indicator_fn {
    ($name:tt, $func:ident) => {
        fn $name(n: usize, samples: &[Sample]) -> Result<IndicatorData, DiError> {
            let mut value = 0.0;
            match $func::new(n) {
                Ok(mut f) => {
                    for sample in samples[samples.len() - n..].iter() {
                        value = f.next(sample);
                    }
                }
                Err(_) => (),
            }
            Ok(IndicatorData::Scalar(value))
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
            match $words[1].parse::<usize>() {
                Ok(n) => return Some(Indicator::$func(n)),
                Err(_) => (),
            }
        }
    };
}

pub fn bollinger_bands_s(n: usize, w: f64, samples: &[Sample]) -> Result<IndicatorData, DiError> {
    let mut r: Vec<Vec<f64>> = vec![Vec::new(), Vec::new(), Vec::new()];
    let mut bb = BollingerBands::new(n, w).unwrap();
    for sample in samples {
        let cur = bb.next(sample);
        r[0].push(cur.lower);
        r[1].push(cur.average);
        r[2].push(cur.upper);
    }
    Ok(IndicatorData::Matrix(r))
}

pub fn bollinger_bands(n: usize, w: f64, samples: &[Sample]) -> Result<IndicatorData, DiError> {
    match bollinger_bands_s(n, w, &samples[samples.len().saturating_sub(n)..]) {
        Ok(IndicatorData::Matrix(r)) => Ok(IndicatorData::Matrix(vec![
            vec![r[0].last().unwrap().clone()],
            vec![r[1].last().unwrap().clone()],
            vec![r[2].last().unwrap().clone()],
        ])),
        Ok(_) => Err(DiError::Error),
        Err(e) => Err(e),
    }
}

fn _round(nums: (f64, f64, f64)) -> (f64, f64, f64) {
    let n0 = (nums.0 * 100.0).round() / 100.0;
    let n1 = (nums.1 * 100.0).round() / 100.0;
    let n2 = (nums.2 * 100.0).round() / 100.0;
    (n0, n1, n2)
}

pub fn macd_s(
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
    samples: &[Sample],
) -> Result<IndicatorData, DiError> {
    let mut r: Vec<Vec<f64>> = vec![Vec::new(), Vec::new()];
    let mut macd =
        MovingAverageConvergenceDivergence::new(fast_period, slow_period, signal_period).unwrap();
    for sample in samples {
        let cur = macd.next(sample);
        // (macd, signal, histogram)
        let ro = (cur.macd, cur.signal); //round(cur.into());
        r[0].push(ro.0);
        r[1].push(ro.1);
    }
    Ok(IndicatorData::Matrix(r))
}

pub fn macd(
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
    samples: &[Sample],
) -> Result<IndicatorData, DiError> {
    match macd_s(
        fast_period,
        slow_period,
        signal_period,
        &samples[samples.len() - slow_period..],
    ) {
        Ok(IndicatorData::Matrix(r)) => Ok(IndicatorData::Matrix(vec![
            vec![r[0].last().unwrap().clone()],
            vec![r[1].last().unwrap().clone()],
        ])),
        Ok(_) => Err(DiError::Error),
        Err(e) => Err(e),
    }
}

fn check_resistance(a: &Sample, b: &Sample, c: &Sample, is_support: bool) -> Option<f64> {
    if is_support {
        let t_0 = f64::min(a.open, a.close);
        let t_1 = f64::min(b.open, b.close);
        let t_2 = f64::min(c.open, c.close);
        if t_1 <= t_0 && t_1 <= t_2 {
            return Some(t_1);
        }
    } else {
        let t_0 = f64::max(a.open, a.close);
        let t_1 = f64::max(b.open, b.close);
        let t_2 = f64::max(c.open, c.close);
        if t_1 >= t_0 && t_1 >= t_2 {
            return Some(t_1);
        }
    }
    None
}

pub fn resistance_lines(
    w: f64,
    is_support: bool,
    samples: &[Sample],
) -> Result<IndicatorData, DiError> {
    let mut lines: Vec<(f64, usize)> = Vec::new();
    for i in 1..samples.len().saturating_sub(1) {
        if let Some(value) =
            check_resistance(&samples[i - 1], &samples[i], &samples[i + 1], is_support)
        {
            let mut found = false;
            for line in lines.iter_mut() {
                if f64::abs((line.0 / line.1 as f64 - value) / value) <= w {
                    line.0 += value;
                    line.1 += 1;
                    found = true;
                }
            }
            if !found {
                lines.push((value, 1));
            }
        }
    }

    lines.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    lines.reverse();

    let mut r: Vec<Vec<f64>> = vec![Vec::new(); lines.len()];

    for _ in samples {
        for (i, line) in lines.iter().enumerate() {
            r[i].push(line.0 / line.1 as f64);
        }
    }

    Ok(IndicatorData::Matrix(r))
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
        "MACD" => {
            if let (Ok(fp), Ok(sp), Ok(ss)) = (
                words[1].parse::<usize>(),
                words[2].parse::<usize>(),
                words[3].parse::<usize>(),
            ) {
                return Some(Indicator::MovingAverageConvergenceDivergence((fp, sp, ss)));
            }
        }
        "BBANDS" => match words[1].parse::<usize>() {
            Ok(n) => return Some(Indicator::BollingerBands((n, 2.0.into()))),
            Err(_) => (),
        },
        "RL" => match words[1].parse::<f64>() {
            Ok(w) => return Some(Indicator::ResistanceLines(w.into())),
            Err(_) => (),
        },
        "SL" => match words[1].parse::<f64>() {
            Ok(w) => return Some(Indicator::SupportLines(w.into())),
            Err(_) => (),
        },
        _ => (),
    };
    None
}

impl Indicator {
    pub fn source(&self) -> IndicatorSource {
        match &self {
            Self::ExponentialMovingAverage(_) => IndicatorSource::Candle,
            Self::SimpleMovingAverage(_) => IndicatorSource::Candle,
            Self::StandardDeviation(_) => IndicatorSource::Candle,
            Self::RelativeStrengthIndex(_) => IndicatorSource::Volume,
            Self::BollingerBands(_) => IndicatorSource::Candle,
            Self::MovingAverageConvergenceDivergence(_) => IndicatorSource::Candle,
            Self::ResistanceLines(_) => IndicatorSource::Candle,
            Self::SupportLines(_) => IndicatorSource::Candle,
        }
    }

    pub fn domain(&self) -> IndicatorDomain {
        match &self {
            Self::ExponentialMovingAverage(_) => IndicatorDomain::Price,
            Self::SimpleMovingAverage(_) => IndicatorDomain::Price,
            Self::StandardDeviation(_) => IndicatorDomain::Cartesian,
            Self::RelativeStrengthIndex(_) => IndicatorDomain::Percent,
            Self::BollingerBands(_) => IndicatorDomain::Price,
            Self::MovingAverageConvergenceDivergence(_) => IndicatorDomain::Cartesian,
            Self::SupportLines(_) => IndicatorDomain::Price,
            Self::ResistanceLines(_) => IndicatorDomain::Price,
        }
    }

    pub fn compute_series(&self, samples: &[Sample]) -> Result<IndicatorData, DiError> {
        match &self {
            Self::ExponentialMovingAverage(n) => exponential_moving_average_s(*n as usize, samples),
            Self::SimpleMovingAverage(n) => simple_moving_average_s(*n as usize, samples),
            Self::StandardDeviation(n) => standard_deviation_s(*n as usize, samples),
            Self::RelativeStrengthIndex(n) => relative_strength_index_s(*n as usize, samples),
            Self::BollingerBands((n, w)) => bollinger_bands_s(*n, w.value, samples),
            Self::MovingAverageConvergenceDivergence((fp, sp, ss)) => {
                macd_s(*fp, *sp, *ss, samples)
            }
            Self::ResistanceLines(w) => resistance_lines(w.value, false, samples),
            Self::SupportLines(w) => resistance_lines(w.value, true, samples),
        }
    }
    pub fn compute(&self, samples: &[Sample]) -> Result<IndicatorData, DiError> {
        match &self {
            Self::ExponentialMovingAverage(n) => exponential_moving_average(*n as usize, samples),
            Self::SimpleMovingAverage(n) => simple_moving_average(*n as usize, samples),
            Self::StandardDeviation(n) => standard_deviation(*n as usize, samples),
            Self::RelativeStrengthIndex(n) => relative_strength_index(*n as usize, samples),
            Self::BollingerBands((n, w)) => bollinger_bands(*n, w.value, samples),
            Self::MovingAverageConvergenceDivergence((fp, sp, ss)) => macd(*fp, *sp, *ss, samples),
            Self::ResistanceLines(w) => resistance_lines(w.value, false, samples),
            Self::SupportLines(w) => resistance_lines(w.value, true, samples),
        }
    }
    pub fn to_string(&self) -> String {
        match &self {
            Self::ExponentialMovingAverage(n) => format!("EMA {:?}", n),
            Self::SimpleMovingAverage(n) => format!("SMA {:?}", n),
            Self::StandardDeviation(n) => format!("sdev {:?}", n),
            Self::RelativeStrengthIndex(n) => format!("rsi {:?}", n),
            Self::BollingerBands((n, w)) => format!("B-Bands {:?} {:?}", n, w.value),
            Self::MovingAverageConvergenceDivergence((fp, sp, ss)) => {
                format!("MACD {:?} {:?} {:?}", fp, sp, ss)
            }
            Self::ResistanceLines(w) => {
                format!("RL {:?}", w.value)
            }
            Self::SupportLines(w) => {
                format!("SL {:?}", w.value)
            }
        }
    }
}

impl Indicators {
    pub fn add(&mut self, indicator: &Indicator) {
        for i in &self.indicators {
            if i == indicator {
                return;
            }
        }
        self.indicators.push(indicator.clone());
    }
}
