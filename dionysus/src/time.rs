use chrono::{DateTime, TimeDelta, Utc};
use regex::Regex;
use std;
use std::hash::{Hash, Hasher};

/// Time measurement unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeUnit {
    Sec(u32),
    Min(u32),
    Hour(u32),
    Day(u32),
    Week(u32),
    Month(u32),
    Year(u32),
    Unit(u32),
}

impl Default for TimeUnit {
    fn default() -> Self {
        TimeUnit::Hour(1)
    }
}

impl TimeUnit {
    pub fn from_name(name: &str) -> Self {
        let re = Regex::new(r"([0-9]+)([a-zA-Z]+)$").unwrap();
        for (_, [frequency, unit]) in re.captures_iter(name).map(|c| c.extract()) {
            let n = frequency.parse::<u32>().unwrap_or(0);
            match unit {
                "s" => return TimeUnit::Sec(n),
                "m" => return TimeUnit::Min(n),
                "h" => return TimeUnit::Hour(n),
                "d" => return TimeUnit::Day(n),
                "wk" => return TimeUnit::Week(n),
                "mo" => return TimeUnit::Month(n),
                "y" => return TimeUnit::Year(n),
                _ => (),
            };
        }
        TimeUnit::Unit(0)
    }
    pub fn name(&self) -> String {
        match self {
            TimeUnit::Sec(n) => format!("{:?}s", n).to_string(),
            TimeUnit::Min(n) => format!("{:?}m", n).to_string(),
            TimeUnit::Hour(n) => format!("{:?}h", n).to_string(),
            TimeUnit::Day(n) => format!("{:?}d", n).to_string(),
            TimeUnit::Week(n) => format!("{:?}wk", n).to_string(),
            TimeUnit::Month(n) => format!("{:?}mo", n).to_string(),
            TimeUnit::Year(n) => format!("{:?}y", n).to_string(),
            TimeUnit::Unit(n) => format!("{:?}u", n).to_string(),
        }
    }
    pub fn count(self) -> u32 {
        match self {
            TimeUnit::Unit(n) => n,
            TimeUnit::Sec(n) => n,
            TimeUnit::Min(n) => n,
            TimeUnit::Hour(n) => n,
            TimeUnit::Day(n) => n,
            TimeUnit::Week(n) => n,
            TimeUnit::Month(n) => n,
            TimeUnit::Year(n) => n,
        }
    }
    pub fn num_seconds(&self) -> i64 {
        match self {
            TimeUnit::Unit(n) => (1 * n) as i64,
            TimeUnit::Sec(n) => (1 * n) as i64,
            TimeUnit::Min(n) => (60 * n) as i64,
            TimeUnit::Hour(n) => (60 * 60 * n) as i64,
            TimeUnit::Day(n) => (24 * 60 * 60 * n) as i64,
            TimeUnit::Week(n) => (7 * 24 * 60 * 60 * n) as i64,
            TimeUnit::Month(n) => (31 * 24 * 60 * 60 * n) as i64,
            TimeUnit::Year(n) => (365 * 24 * 60 * 60 * n) as i64,
        }
    }
}

impl Hash for TimeUnit {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name().hash(state);
    }
}

#[derive(Debug, Copy, Default, Clone)]
pub struct TimeWindow {
    pub resolution: TimeUnit,
    pub count: i64,
}

impl TimeWindow {
    pub fn minutes(n: i64) -> TimeWindow {
        TimeWindow {
            resolution: TimeUnit::Min(1),
            count: n,
        }
    }
    pub fn days(n: i64) -> TimeWindow {
        TimeWindow {
            resolution: TimeUnit::Day(1),
            count: n,
        }
    }
    pub fn num_seconds(&self) -> i64 {
        self.resolution.num_seconds() * self.count
    }
    pub fn num_minutes(&self) -> i64 {
        self.num_seconds() / 60
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Date {
    utc: DateTime<Utc>,
}

impl Date {
    pub fn from(date: DateTime<Utc>) -> Self {
        Date { utc: date }
    }
    pub fn from_timestamp(timestamp: u64) -> Self {
        let date = DateTime::from_timestamp(timestamp as i64, 0).unwrap();
        Date {
            utc: date.with_timezone(&Utc),
        }
    }
    pub fn now() -> Date {
        Date { utc: Utc::now() }
    }
    pub fn timestamp(&self) -> i64 {
        self.utc.timestamp()
    }
}

impl std::ops::Sub<TimeWindow> for Date {
    type Output = Date;

    fn sub(self, rhs: TimeWindow) -> Date {
        Date {
            utc: self.utc - TimeDelta::try_seconds(rhs.num_seconds()).unwrap(),
        }
    }
}

impl std::ops::Sub<Date> for Date {
    type Output = TimeDelta;

    fn sub(self, rhs: Date) -> TimeDelta {
        self.utc - rhs.utc
    }
}

impl std::ops::SubAssign<TimeWindow> for Date {
    fn sub_assign(&mut self, rhs: TimeWindow) {
        self.utc -= TimeDelta::try_seconds(rhs.num_seconds()).unwrap()
    }
}

impl std::ops::Add<TimeWindow> for Date {
    type Output = Date;

    fn add(self, rhs: TimeWindow) -> Self::Output {
        Date {
            utc: self.utc + TimeDelta::try_seconds(rhs.num_seconds()).unwrap(),
        }
    }
}

impl std::ops::AddAssign<TimeWindow> for Date {
    fn add_assign(&mut self, rhs: TimeWindow) {
        self.utc += TimeDelta::try_seconds(rhs.num_seconds()).unwrap()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Period {
    pub duration: TimeWindow,
    start: Date,
}

impl Period {
    pub fn last(time_period: TimeWindow) -> Period {
        let end = Date::now();
        Period {
            duration: time_period,
            start: end - time_period,
        }
    }
    pub fn start(&self) -> Date {
        self.start.clone()
    }
    pub fn end(&self) -> Date {
        self.start + self.duration
    }
}

#[cfg(test)]
mod tests {
    use crate::time::TimeUnit;

    use super::{Date, Period, TimeWindow};

    #[test]
    fn test_time_unit() {
        assert_eq!(TimeUnit::Min(1).name(), "1m".to_string());
    }

    #[test]
    fn test_period() {
        let period = Period::last(super::TimeWindow::days(100));
        assert_eq!(
            period.duration.num_seconds(),
            (period.end() - period.start()).num_seconds()
        );
    }

    #[test]
    fn test_duration() {
        let start = Date::now() - TimeWindow::days(1);
        let end = Date::now();
        let delta = end - start;
        assert_eq!(delta.num_seconds(), 24 * 60 * 60);
    }
}
