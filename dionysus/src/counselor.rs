use slog::slog_info;
use slog_scope;

use crate::{
    finance::{DiError, OrderType, Quote, Sample, TimeInForce, Token},
    indicators::{BollingerBandsAttributes, Indicator, IndicatorData},
    time::Date,
    INFO,
};
use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Crossover {
    Equal,
    Bellow,
    Over,
    CrossingUpwards,
    CrossingDownwards,
}

impl Crossover {
    pub fn from(curr: Ordering, prev: Ordering) -> Crossover {
        match &prev {
            Ordering::Greater => match &curr {
                Ordering::Greater => Crossover::Over,
                _ => Crossover::CrossingDownwards,
            },
            Ordering::Equal => match &curr {
                Ordering::Greater => Crossover::CrossingUpwards,
                Ordering::Equal => Crossover::Equal,
                Ordering::Less => Crossover::CrossingDownwards,
            },
            Ordering::Less => match &curr {
                Ordering::Less => Crossover::Bellow,
                _ => Crossover::CrossingUpwards,
            },
        }
    }
    pub fn signal(&self) -> i32 {
        match &self {
            Self::Bellow => -1,
            Self::Over => 1,
            Self::Equal => 0,
            Self::CrossingUpwards => 2,
            Self::CrossingDownwards => -2,
        }
    }
}

pub fn cross_from_ord(ord: &[Ordering]) -> Vec<Crossover> {
    ord.iter()
        .enumerate()
        .map(|(i, o)| {
            if i > 0 {
                Crossover::from(*o, ord[i - 1])
            } else {
                if *o == Ordering::Greater {
                    Crossover::Over
                } else {
                    Crossover::Bellow
                }
            }
        })
        .collect()
}

pub fn compute_crossover_s<T, F>(a: &[T], b: &[T], f: F) -> Vec<Crossover>
where
    F: Fn(&T, &T) -> Ordering,
{
    let ord: Vec<Ordering> = a.iter().enumerate().map(|(i, t)| f(&t, &b[i])).collect();
    cross_from_ord(&ord[..])
}

pub fn compute_crossover<T, F>(a: &[T], b: &[T], f: F) -> Crossover
where
    F: Fn(&T, &T) -> Ordering,
{
    compute_crossover_s(a, b, f).last().unwrap().clone()
}

pub fn compute_zero_cross_s(curve: &[f64]) -> Vec<Crossover> {
    let zero = 0.0;
    let ord: Vec<Ordering> = curve
        .iter()
        .map(|c| c.partial_cmp(&zero).unwrap())
        .collect();
    cross_from_ord(&ord[..])
}

pub fn compute_zero_cross(curve: &[f64]) -> Crossover {
    compute_zero_cross_s(curve).last().unwrap().clone()
}

/// A signal represents the sentiment of an strategy.
#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub enum Signal {
    Buy,
    Sell,
    #[default]
    None,
}

#[derive(Default, Debug, Clone)]
pub struct Advice {
    pub signal: Signal,
    pub order_type: OrderType,
    pub stop_price: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub tif: TimeInForce,
}

#[derive(Clone, Default, Debug)]
pub enum Counselor {
    #[default]
    Trace,
    MeanReversion(usize),
    MACDCrossover((usize, usize, usize)),
    MACDZeroCross((usize, usize, usize)),
    EMACross((usize, usize)),
}

macro_rules! match_oracle {
    ($func:ident, $words:expr) => {
        if $words.len() == 2 {
            match $words[1].parse::<usize>() {
                Ok(n) => return Some(Counselor::$func(n)),
                Err(_) => (),
            }
        } else {
            INFO!("{:?}: arguments not found!", $words);
        }
    };
}

pub fn match_oracle_from_text(words: &[&str]) -> Option<Counselor> {
    match words[0].to_uppercase().as_str() {
        "MEAN-REVERSION" => {
            match_oracle!(MeanReversion, words)
        }
        "MACD-CROSSOVER" => {
            if let (Ok(fp), Ok(sp), Ok(ss)) = (
                words[1].parse::<usize>(),
                words[2].parse::<usize>(),
                words[3].parse::<usize>(),
            ) {
                return Some(Counselor::MACDCrossover((fp, sp, ss)));
            }
        }
        "MACD-ZERO-CROSS" => {
            if let (Ok(fp), Ok(sp), Ok(ss)) = (
                words[1].parse::<usize>(),
                words[2].parse::<usize>(),
                words[3].parse::<usize>(),
            ) {
                return Some(Counselor::MACDZeroCross((fp, sp, ss)));
            }
        }
        "EMA-CROSS" => {
            if let (Ok(fp), Ok(sp)) = (words[1].parse::<usize>(), words[2].parse::<usize>()) {
                return Some(Counselor::EMACross((fp, sp)));
            }
        }
        "TRACE" => return Some(Counselor::Trace),
        _ => (),
    };
    None
}

impl Counselor {
    pub fn required_samples(&self) -> usize {
        match self {
            Counselor::Trace => 0,
            Counselor::MeanReversion(n) => *n,
            Counselor::MACDCrossover((_, sp, _)) => *sp,
            Counselor::MACDZeroCross((_, sp, _)) => *sp,
            Counselor::EMACross((_, sp)) => *sp,
        }
    }
    pub fn run(&self, quote: &Quote, history: &[Sample]) -> Result<Advice, DiError> {
        match self {
            Counselor::Trace => run_trace(quote),
            Counselor::MeanReversion(n) => run_mean_reversion(*n, quote, history),
            Counselor::MACDCrossover((fp, sp, ss)) => {
                run_macd_crossover(*fp, *sp, *ss, quote, history)
            }
            Counselor::MACDZeroCross((fp, sp, ss)) => {
                run_macd_zero_cross(*fp, *sp, *ss, quote, history)
            }
            Counselor::EMACross((fp, sp)) => run_ema_cross(*fp, *sp, quote, history),
        }
    }
    pub fn run_series(&self, samples: &[Sample]) -> Result<Vec<Advice>, DiError> {
        let n = self.required_samples();
        let mut advices: Vec<Advice> = vec![Advice::default(); samples.len()];
        for i in n..samples.len() {
            advices[i] = self
                .run(
                    &Quote {
                        token: Token::default(),
                        bid: Some(samples[i].close),
                        ask: Some(samples[i].close),
                        biddate: Date::from_timestamp(samples[i].timestamp),
                        askdate: Date::from_timestamp(samples[i].timestamp),
                    },
                    &samples[..i + 1],
                )
                .unwrap();
        }
        Ok(advices)
    }
    pub fn indicators(&self) -> Vec<Indicator> {
        match self {
            Counselor::MeanReversion(n) => {
                vec![Indicator::BollingerBands(BollingerBandsAttributes {
                    n: *n,
                    w: 2.0,
                })]
            }
            Counselor::MACDCrossover((fp, sp, ss)) => {
                vec![Indicator::MovingAverageConvergenceDivergence((
                    *fp, *sp, *ss,
                ))]
            }
            Counselor::MACDZeroCross((fp, sp, ss)) => {
                vec![Indicator::MovingAverageConvergenceDivergence((
                    *fp, *sp, *ss,
                ))]
            }
            Counselor::EMACross((fp, sp)) => {
                vec![
                    Indicator::ExponentialMovingAverage(*fp),
                    Indicator::ExponentialMovingAverage(*sp),
                ]
            }
            _ => Vec::new(),
        }
    }
    pub fn name(&self) -> String {
        match &self {
            Counselor::Trace => format!("trace"),
            Counselor::MeanReversion(n) => format!("mean-reversion({:?})", n),
            Counselor::MACDCrossover((fp, sp, ss)) => {
                format!("macd-crossover({}, {}, {})", fp, sp, ss)
            }
            Counselor::MACDZeroCross((fp, sp, ss)) => {
                format!("macd-zero-cross({}, {}, {})", fp, sp, ss)
            }
            Counselor::EMACross((fp, sp)) => {
                format!("ema-cross({}, {})", fp, sp)
            }
        }
    }
}

fn run_trace(quote: &Quote) -> Result<Advice, DiError> {
    let mut advice = Advice::default();
    advice.stop_price = quote.ask.unwrap_or(-1.0);
    Ok(advice)
}

fn run_mean_reversion(n: usize, quote: &Quote, history: &[Sample]) -> Result<Advice, DiError> {
    let bband_i = Indicator::BollingerBands(BollingerBandsAttributes { n, w: 2.0 });

    let upper: f64;
    let lower: f64;
    match bband_i.compute(history) {
        Ok(IndicatorData::Matrix(m)) => {
            lower = m[0][0];
            upper = m[2][0];
        }
        Ok(_) => return Err(DiError::Error),
        Err(e) => return Err(e),
    };

    let buy = quote.ask.unwrap_or(0.0) < lower;
    let sell = quote.ask.unwrap_or(0.0) > upper;

    let mut advice = Advice::default();
    if buy {
        advice.stop_price = lower;
        advice.stop_loss = lower;
        advice.signal = Signal::Buy;
    } else if sell {
        advice.stop_price = upper;
        advice.stop_loss = upper;
        advice.signal = Signal::Sell;
    }

    Ok(advice)
}

fn run_macd_crossover(
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
    _quote: &Quote,
    history: &[Sample],
) -> Result<Advice, DiError> {
    let macd_i =
        Indicator::MovingAverageConvergenceDivergence((fast_period, slow_period, signal_period));
    let mut crossover = Crossover::Equal;
    if let Ok(IndicatorData::Matrix(macd)) = macd_i.compute_series(history) {
        crossover = compute_crossover(&macd[0][..], &macd[1][..], |a, b| {
            a.partial_cmp(&b).unwrap()
        });
    }
    let last_sample = history.last().unwrap();

    let mut advice = Advice::default();
    match crossover {
        Crossover::CrossingUpwards => {
            advice.signal = Signal::Buy;
            advice.stop_price = last_sample.high;
            advice.stop_loss = last_sample.low;
            advice.take_profit = advice.stop_price + (advice.stop_price - advice.stop_loss);
        }
        Crossover::CrossingDownwards => {
            advice.signal = Signal::Sell;
            advice.stop_price = last_sample.low;
            advice.stop_loss = last_sample.high;
            advice.take_profit = advice.stop_price - (advice.stop_loss - advice.stop_price);
        }
        _ => (),
    }

    Ok(advice)
}

fn run_macd_zero_cross(
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
    _quote: &Quote,
    history: &[Sample],
) -> Result<Advice, DiError> {
    let macd_i =
        Indicator::MovingAverageConvergenceDivergence((fast_period, slow_period, signal_period));
    let mut crossover = Crossover::Equal;
    if let Ok(IndicatorData::Matrix(macd)) = macd_i.compute_series(history) {
        crossover = compute_zero_cross(&macd[0][..]);
    }
    let last_sample = history.last().unwrap();

    let mut advice = Advice::default();
    match crossover {
        Crossover::CrossingUpwards => {
            advice.signal = Signal::Buy;
            advice.stop_price = last_sample.high;
            advice.stop_loss = last_sample.low;
            advice.take_profit = advice.stop_price + (advice.stop_price - advice.stop_loss);
        }
        Crossover::CrossingDownwards => {
            advice.signal = Signal::Sell;
            advice.stop_price = last_sample.low;
            advice.stop_loss = last_sample.high;
            advice.take_profit = advice.stop_price - (advice.stop_loss - advice.stop_price);
        }
        _ => (),
    }

    Ok(advice)
}

fn run_ema_cross(
    fast_period: usize,
    slow_period: usize,
    _quote: &Quote,
    history: &[Sample],
) -> Result<Advice, DiError> {
    let last_sample = history.last().unwrap();
    let fast_ema_i = Indicator::ExponentialMovingAverage(fast_period);
    let slow_ema_i = Indicator::ExponentialMovingAverage(slow_period);

    let fast_ema;
    match fast_ema_i.compute_series(history) {
        Ok(IndicatorData::Vector(v)) => fast_ema = v,
        Err(e) => return Err(e),
        _ => return Err(DiError::Error),
    }

    let slow_ema;
    match slow_ema_i.compute_series(history) {
        Ok(IndicatorData::Vector(v)) => slow_ema = v,
        Err(e) => return Err(e),
        _ => return Err(DiError::Error),
    }

    let crossover = compute_crossover(&fast_ema[..], &slow_ema[..], |a, b| {
        a.partial_cmp(b).unwrap()
    });

    let mut advice = Advice::default();

    match crossover {
        Crossover::CrossingUpwards => {
            advice.signal = Signal::Buy;
            advice.stop_loss = slow_ema.last().unwrap().clone();
            advice.stop_price = last_sample.high;
            advice.take_profit = advice.stop_price + (advice.stop_price - advice.stop_loss);
        }
        Crossover::CrossingDownwards => {
            advice.signal = Signal::Sell;
            advice.stop_loss = slow_ema.last().unwrap().clone();
            advice.stop_price = last_sample.low;
            advice.take_profit = advice.stop_price - (advice.stop_loss - advice.stop_price);
        }
        _ => (),
    }

    Ok(advice)
}

#[cfg(test)]
mod tests {
    use crate::counselor::Crossover;

    use super::compute_crossover_s;

    #[test]
    fn test_crossover() {
        {
            let a = vec![0, 1, 2, 3, 4];
            let b = vec![4, 3, 2, 1, 0];
            let r = compute_crossover_s(&a[..], &b[..], |ai, bi| ai.cmp(bi));
            assert_eq!(r[0], Crossover::Bellow);
            assert_eq!(r[1], Crossover::Bellow);
            assert_eq!(r[2], Crossover::CrossingUpwards);
            assert_eq!(r[3], Crossover::CrossingUpwards);
            assert_eq!(r[4], Crossover::Over);
            let rr = compute_crossover_s(&b[..], &a[..], |ai, bi| ai.cmp(bi));
            assert_eq!(rr[0], Crossover::Over);
            assert_eq!(rr[1], Crossover::Over);
            assert_eq!(rr[2], Crossover::CrossingDownwards);
            assert_eq!(rr[3], Crossover::CrossingDownwards);
            assert_eq!(rr[4], Crossover::Bellow);
        }
        {
            let a = vec![-2, -1, 0, 0, -1];
            let b = vec![0, 0, 0, 0, 0];
            let r = compute_crossover_s(&a[..], &b[..], |ai, bi| ai.cmp(bi));
            assert_eq!(r[0], Crossover::Bellow);
            assert_eq!(r[1], Crossover::Bellow);
            assert_eq!(r[2], Crossover::CrossingUpwards);
            assert_eq!(r[3], Crossover::Equal);
            assert_eq!(r[4], Crossover::CrossingDownwards);
            let rr = compute_crossover_s(&b[..], &a[..], |ai, bi| ai.cmp(bi));
            assert_eq!(rr[0], Crossover::Over);
            assert_eq!(rr[1], Crossover::Over);
            assert_eq!(rr[2], Crossover::CrossingDownwards);
            assert_eq!(rr[3], Crossover::Equal);
            assert_eq!(rr[4], Crossover::CrossingUpwards);
        }
    }
}
