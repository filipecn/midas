use std::collections::HashMap;

use super::{SentimentClassifier, SentimentScore, score_from_compound};

/// Weights use a VADER-style range, where stronger terms have greater impact.
pub type Lexicon = HashMap<String, f32>;

#[derive(Debug, Clone)]
pub struct LexiconConfig {
    pub terms: Lexicon,
    pub neutral_threshold: f32,
    pub negation_window: usize,
    pub negation_multiplier: f32,
    pub booster_multiplier: f32,
}

impl Default for LexiconConfig {
    fn default() -> Self {
        Self {
            terms: portuguese_finance_lexicon(),
            neutral_threshold: 0.05,
            negation_window: 3,
            negation_multiplier: -0.74,
            booster_multiplier: 1.25,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LexiconClassifier {
    config: LexiconConfig,
}

impl LexiconClassifier {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(config: LexiconConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &LexiconConfig {
        &self.config
    }
}

impl SentimentClassifier for LexiconClassifier {
    fn classify(&self, text: &str) -> SentimentScore {
        let tokens: Vec<String> = text
            .split(|ch: char| !ch.is_alphanumeric() && ch != '-')
            .filter(|token| !token.is_empty())
            .map(str::to_lowercase)
            .collect();

        let mut total = 0.0_f32;
        for (index, token) in tokens.iter().enumerate() {
            let Some(&weight) = self.config.terms.get(token) else {
                continue;
            };
            let start = index.saturating_sub(self.config.negation_window);
            let context = &tokens[start..index];
            let negated = context.iter().any(|word| is_negation(word));
            let boosted = context.last().is_some_and(|word| is_booster(word));
            let mut adjusted = if boosted {
                weight * self.config.booster_multiplier
            } else {
                weight
            };
            if negated {
                adjusted *= self.config.negation_multiplier;
            }
            total += adjusted;
        }

        // VADER's compound-score normalization keeps scores comparable as the
        // number of matched words changes.
        let compound = if total == 0.0 {
            0.0
        } else {
            total / (total * total + 15.0).sqrt()
        };
        score_from_compound(compound, self.config.neutral_threshold)
    }
}

fn is_negation(word: &str) -> bool {
    matches!(word, "não" | "nao" | "nunca" | "nem" | "sem" | "jamais")
}

fn is_booster(word: &str) -> bool {
    matches!(
        word,
        "muito" | "bastante" | "fortemente" | "significativamente" | "forte"
    )
}

/// Starter Brazilian Portuguese financial vocabulary. Callers can extend or
/// replace this map with `LexiconConfig` for their own terminology and corpus.
pub fn portuguese_finance_lexicon() -> Lexicon {
    [
        ("lucro", 2.8),
        ("lucrativo", 2.7),
        ("lucratividade", 2.5),
        ("crescimento", 2.2),
        ("cresce", 1.9),
        ("alta", 1.8),
        ("valorização", 2.4),
        ("valorizacao", 2.4),
        ("recorde", 2.1),
        ("dividendos", 1.4),
        ("recompra", 1.1),
        ("supera", 1.8),
        ("superou", 1.8),
        ("otimista", 2.0),
        ("positivo", 1.7),
        ("positiva", 1.7),
        ("favorável", 1.8),
        ("favoravel", 1.8),
        ("expansão", 1.7),
        ("expansao", 1.7),
        ("eficiência", 1.4),
        ("eficiencia", 1.4),
        ("queda", -1.8),
        ("cai", -1.7),
        ("recuo", -1.8),
        ("recuou", -1.8),
        ("prejuízo", -3.2),
        ("prejuizo", -3.2),
        ("déficit", -2.6),
        ("deficit", -2.6),
        ("dívida", -1.8),
        ("divida", -1.8),
        ("endividamento", -2.0),
        ("calote", -3.1),
        ("default", -2.8),
        ("rebaixamento", -2.6),
        ("downgrade", -2.4),
        ("crise", -2.8),
        ("risco", -1.2),
        ("incerteza", -1.6),
        ("pessimista", -2.0),
        ("negativo", -1.7),
        ("negativa", -1.7),
        ("multa", -1.6),
        ("investigação", -1.7),
        ("investigacao", -1.7),
        ("fraude", -3.5),
        ("recuperação", 1.0),
        ("recuperacao", 1.0),
        ("estável", 0.4),
        ("estavel", 0.4),
    ]
    .into_iter()
    .map(|(term, weight)| (term.to_owned(), weight))
    .collect()
}
