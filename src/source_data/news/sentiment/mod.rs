//! Sentiment scoring methods for news headlines and summaries.

mod lexicon;
#[cfg(feature = "news-transformer")]
mod transformer;

pub use lexicon::{LexiconClassifier, LexiconConfig, portuguese_finance_lexicon};
#[cfg(feature = "news-transformer")]
pub use transformer::{TransformerClassifier, TransformerLabelMap};

use serde::{Deserialize, Serialize};

use crate::source_data::news::NewsArticle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SentimentLabel {
    Positive,
    Neutral,
    Negative,
}

/// Signed compound score on [-1, 1], plus the discrete label and confidence.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SentimentScore {
    pub label: SentimentLabel,
    pub score: f32,
    pub confidence: f32,
}

pub trait SentimentClassifier {
    fn classify(&self, text: &str) -> SentimentScore;

    fn classify_article(&self, article: &NewsArticle) -> SentimentScore {
        let text = match article.summary.as_deref() {
            Some(summary) if !summary.trim().is_empty() => {
                format!("{}\n{}", article.title, summary)
            }
            _ => article.title.clone(),
        };
        self.classify(&text)
    }
}

/// Runs the configured sentiment methods and elects the result by
/// confidence-weighted vote. The default engine currently uses the Portuguese
/// finance lexicon; additional methods can be registered as they are added.
pub struct SentimentEngine {
    classifiers: Vec<Box<dyn SentimentClassifier>>,
}

impl Default for SentimentEngine {
    fn default() -> Self {
        Self::with_lexicon()
    }
}

impl SentimentEngine {
    pub fn new() -> Self {
        Self {
            classifiers: Vec::new(),
        }
    }

    pub fn with_lexicon() -> Self {
        let mut engine = Self::new();
        engine.add_classifier(LexiconClassifier::new());
        engine
    }

    pub fn add_classifier<C>(&mut self, classifier: C)
    where
        C: SentimentClassifier + 'static,
    {
        self.classifiers.push(Box::new(classifier));
    }

    pub fn classify(&self, text: &str) -> SentimentScore {
        self.elect(
            self.classifiers
                .iter()
                .map(|classifier| classifier.classify(text)),
        )
    }

    pub fn classify_article(&self, article: &NewsArticle) -> SentimentScore {
        self.elect(
            self.classifiers
                .iter()
                .map(|classifier| classifier.classify_article(article)),
        )
    }

    fn elect(&self, scores: impl Iterator<Item = SentimentScore>) -> SentimentScore {
        let scores: Vec<_> = scores.collect();
        if scores.is_empty() {
            return SentimentScore {
                label: SentimentLabel::Neutral,
                score: 0.0,
                confidence: 0.0,
            };
        }

        let mut votes = [0.0_f32; 3];
        let mut strongest = [None; 3];
        for score in scores {
            let index = match score.label {
                SentimentLabel::Positive => 0,
                SentimentLabel::Neutral => 1,
                SentimentLabel::Negative => 2,
            };
            votes[index] += score.confidence.clamp(0.0, 1.0).max(0.01);
            if strongest[index]
                .is_none_or(|current: SentimentScore| score.confidence > current.confidence)
            {
                strongest[index] = Some(score);
            }
        }
        let winner = (0..3)
            .max_by(|left, right| {
                votes[*left].total_cmp(&votes[*right]).then_with(|| {
                    let left_confidence = strongest[*left].map_or(0.0, |score| score.confidence);
                    let right_confidence = strongest[*right].map_or(0.0, |score| score.confidence);
                    left_confidence.total_cmp(&right_confidence)
                })
            })
            .expect("sentiment vote has three labels");
        strongest[winner].expect("winning label has a score")
    }
}

pub(crate) fn score_from_compound(score: f32, neutral_threshold: f32) -> SentimentScore {
    let score = score.clamp(-1.0, 1.0);
    let label = if score > neutral_threshold {
        SentimentLabel::Positive
    } else if score < -neutral_threshold {
        SentimentLabel::Negative
    } else {
        SentimentLabel::Neutral
    };
    SentimentScore {
        label,
        score,
        confidence: score.abs(),
    }
}
