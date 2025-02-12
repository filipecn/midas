use chrono::{DateTime, Local, TimeDelta};
use std;

/// Time measurement unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeUnit {
    Min,
    Hour,
    Day,
    Week,
    Month,
    Year,
}

pub fn duration_in_seconds(time_unit: TimeUnit) -> i64 {
    match time_unit {
        TimeUnit::Min => 60,
        TimeUnit::Hour => 60 * 60,
        TimeUnit::Day => 24 * 60 * 60,
        TimeUnit::Week => 7 * 24 * 60 * 60,
        TimeUnit::Month => 30 * 24 * 60 * 60,
        TimeUnit::Year => 365 * 24 * 60 * 60,
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Resolution {
    pub unit: TimeUnit,
    pub frequency: i64,
}

impl Resolution {
    pub fn day() -> Resolution {
        Resolution {
            unit: TimeUnit::Day,
            frequency: 1,
        }
    }
    pub fn num_seconds(&self) -> i64 {
        duration_in_seconds(self.unit) * self.frequency
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Duration {
    pub resolution: Resolution,
    pub count: i64,
}

impl Duration {
    pub fn days(n: i64) -> Duration {
        Duration {
            resolution: Resolution::day(),
            count: n,
        }
    }
    pub fn num_seconds(&self) -> i64 {
        self.resolution.num_seconds() * self.count
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Date {
    pub local: DateTime<Local>,
}

impl Date {
    pub fn from(date: DateTime<Local>) -> Self {
        Date { local: date }
    }
    pub fn from_timestamp(timestamp: i64) -> Self {
        let date = DateTime::from_timestamp(timestamp, 0).unwrap();
        Date {
            local: date.with_timezone(&Local),
        }
    }
    pub fn now() -> Date {
        Date {
            local: Local::now(),
        }
    }
}

impl std::ops::Sub<Duration> for Date {
    type Output = Date;

    fn sub(self, rhs: Duration) -> Date {
        Date {
            local: self.local - TimeDelta::try_seconds(rhs.num_seconds()).unwrap(),
        }
    }
}

impl std::ops::SubAssign<Duration> for Date {
    fn sub_assign(&mut self, rhs: Duration) {
        self.local -= TimeDelta::try_seconds(rhs.num_seconds()).unwrap()
    }
}

impl std::ops::Add<Duration> for Date {
    type Output = Date;

    fn add(self, rhs: Duration) -> Self::Output {
        Date {
            local: self.local + TimeDelta::try_seconds(rhs.num_seconds()).unwrap(),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Period {
    pub duration: Duration,
    pub start: Date,
}

impl Period {
    pub fn last(time_period: Duration) -> Period {
        let end = Date::now();
        Period {
            duration: time_period,
            start: end - time_period,
        }
    }
    pub fn end(&self) -> Date {
        self.start + self.duration
    }
}
