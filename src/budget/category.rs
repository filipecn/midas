use crate::budget::{
    Category, Transaction,
    data::{Categories, CategorizationRules},
};
use regex::Regex;

pub struct Classifier {
    expense_rules: Vec<(Regex, Category)>,
    income_rules: Vec<(Regex, Category)>,
}

pub struct HintData {
    pub category: Option<String>,
    pub category_id: Option<String>,
}

impl Classifier {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut expense_rules: Vec<(Regex, Category)> = Vec::new();
        let mut income_rules: Vec<(Regex, Category)> = Vec::new();
        let mut err = None;

        for (rules, target) in [
            (CategorizationRules::expense_rules(), &mut expense_rules),
            (CategorizationRules::income_rules(), &mut income_rules),
        ] {
            for rule in rules {
                if let Some(category) = Categories::get(&rule.category) {
                    match Regex::new(&rule.pattern) {
                        Ok(re) => target.push((re, category)),
                        Err(e) => err = Some(e),
                    }
                }
            }
        }
        if let Some(e) = err {
            return Err(Box::new(e));
        }

        Ok(Self {
            expense_rules,
            income_rules,
        })
    }

    pub fn label_with_hint(&self, transaction: &Transaction, hint: HintData) -> Category {
        if transaction.amount_in_cents > 0 {
            // match against income categories only
            for rule in self.income_rules.iter() {
                if rule.0.is_match(&transaction.description) {
                    return rule.1.clone();
                }
            }
            return Categories::income();
        }
        for rule in self.expense_rules.iter() {
            if rule.0.is_match(&transaction.description) {
                return rule.1.clone();
            }
        }
        // get from hints
        Categories::map()
            .into_iter()
            .find(|mapping| {
                hint.category.as_deref().is_some_and(|category| {
                    category == mapping.source_category_id
                        || category == mapping.source_category_name
                }) || hint.category_id.as_deref().is_some_and(|category_id| {
                    category_id == mapping.source_category_id
                        || category_id == mapping.source_category_name
                })
            })
            .map(|mapping| {
                Categories::get_or_uncategorized(if mapping.target_subcategory.is_empty() {
                    &mapping.target_category
                } else {
                    &mapping.target_subcategory
                })
            })
            .unwrap_or_else(Category::uncategorized)
    }

    pub fn label(&self, transaction: &Transaction) -> Category {
        self.label_with_hint(
            transaction,
            HintData {
                category: None,
                category_id: None,
            },
        )
    }
}
