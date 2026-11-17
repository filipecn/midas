//! Optional local transformer backend. Enable with `--features news-transformer`.
//!
//! The selected model must be exported in rust-bert's Torch `.ot` format, and
//! provided with its compatible config and tokenizer files. Hugging Face
//! `.bin`/`.safetensors` weights may need conversion before rust-bert can load
//! them. No model is downloaded automatically.

use std::path::Path;

use rust_bert::{
    pipelines::{
        common::{ModelResource, ModelType},
        sequence_classification::{SequenceClassificationConfig, SequenceClassificationModel},
    },
    resources::LocalResource,
};
use thiserror::Error;

use super::{SentimentClassifier, SentimentLabel, SentimentScore};

#[derive(Debug, Clone)]
pub struct TransformerLabelMap {
    pub positive: String,
    pub neutral: String,
    pub negative: String,
}

impl Default for TransformerLabelMap {
    fn default() -> Self {
        Self {
            positive: "positive".into(),
            neutral: "neutral".into(),
            negative: "negative".into(),
        }
    }
}

#[derive(Debug, Error)]
pub enum TransformerError {
    #[error("rust-bert model initialization failed: {0}")]
    Model(#[from] rust_bert::RustBertError),
}

pub struct TransformerClassifier {
    model: SequenceClassificationModel,
    labels: TransformerLabelMap,
}

impl TransformerClassifier {
    /// Load a local BERT-compatible model directory containing `model.ot`,
    /// `config.json`, and `vocab.txt`.
    pub fn from_local_bert(
        model_dir: impl AsRef<Path>,
        labels: TransformerLabelMap,
    ) -> Result<Self, TransformerError> {
        let model_dir = model_dir.as_ref();
        let config = SequenceClassificationConfig::new(
            ModelType::Bert,
            ModelResource::Torch(Box::new(LocalResource::from(model_dir.join("model.ot")))),
            LocalResource::from(model_dir.join("config.json")),
            LocalResource::from(model_dir.join("vocab.txt")),
            None::<LocalResource>,
            false,
            None,
            None,
        );
        let model = SequenceClassificationModel::new(config)?;
        Ok(Self { model, labels })
    }
}

impl SentimentClassifier for TransformerClassifier {
    fn classify(&self, text: &str) -> SentimentScore {
        let prediction = self.model.predict(&[text]);
        let Some(label) = prediction.first() else {
            return SentimentScore {
                label: SentimentLabel::Neutral,
                score: 0.0,
                confidence: 0.0,
            };
        };
        let predicted = label.text.trim();
        let confidence = label.score.clamp(0.0, 1.0) as f32;
        let (sentiment, score) = if predicted.eq_ignore_ascii_case(&self.labels.positive) {
            (SentimentLabel::Positive, confidence)
        } else if predicted.eq_ignore_ascii_case(&self.labels.negative) {
            (SentimentLabel::Negative, -confidence)
        } else if predicted.eq_ignore_ascii_case(&self.labels.neutral) {
            (SentimentLabel::Neutral, 0.0)
        } else {
            (SentimentLabel::Neutral, 0.0)
        };
        SentimentScore {
            label: sentiment,
            score,
            confidence,
        }
    }
}
