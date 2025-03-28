use crate::counselor::{Advice, Counselor, Signal};
use crate::finance::{DiError, Quote, Sample};
use crate::time::TimeWindow;

#[derive(Default, Debug)]
pub struct Decision {
    pub advice: Advice,
    pub pct: f64,
}

#[derive(Default, Clone, Debug)]
pub enum Oracle {
    #[default]
    Delphi,
    Dodona,
}

impl Oracle {
    pub fn see(
        &self,
        quote: &Quote,
        history: &[Sample],
        counselors: &[Counselor],
    ) -> Result<Decision, DiError> {
        match self {
            Oracle::Delphi => {
                for counselor in counselors.iter() {
                    if let Ok(advice) = counselor.run(quote, history) {
                        if advice.signal != Signal::None {
                            return Ok(Decision { advice, pct: 100.0 });
                        }
                    }
                }
            }
            Oracle::Dodona => (),
        }
        Ok(Decision::default())
    }

    pub fn name(&self) -> String {
        match &self {
            Self::Delphi => format!("Delphi"),
            Self::Dodona => format!("Dodona"),
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct Strategy {
    pub oracle: Oracle,
    pub counselors: Vec<Counselor>,
    pub duration: TimeWindow,
}

impl Strategy {
    pub fn run(&self, quote: &Quote, history: &[Sample]) -> Result<Decision, DiError> {
        self.oracle.see(quote, history, &self.counselors)
    }

    pub fn name(&self) -> String {
        format!("{} {}", self.oracle.name(), self.duration.resolution.name())
    }
}
