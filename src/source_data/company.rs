//! Shared company identity and name matching across data providers.

use crate::source_data::{b3::B3Issuer, cvm::companies::CompanyRegistration};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompanyIdentity {
    pub legal_name: Option<String>,
    pub trading_name: Option<String>,
    pub cnpj: Option<String>,
    pub cvm_code: Option<String>,
    pub b3_issuer_code: Option<String>,
    aliases: Vec<String>,
}

impl CompanyIdentity {
    pub fn from_b3(issuer: &B3Issuer) -> Self {
        let mut identity = Self {
            legal_name: Some(issuer.name.clone()),
            trading_name: nonempty(&issuer.trading_name),
            b3_issuer_code: Some(issuer.code.clone()),
            ..Self::default()
        };
        identity.add_alias(&issuer.name);
        identity.add_alias(&issuer.trading_name);
        identity
    }

    pub fn add_registration(&mut self, company: &CompanyRegistration) {
        self.legal_name = nonempty(&company.legal_name).or(self.legal_name.take());
        self.trading_name = company
            .trading_name
            .clone()
            .filter(|name| !name.trim().is_empty())
            .or(self.trading_name.take());
        self.cnpj = nonempty(&company.cnpj);
        self.cvm_code = nonempty(&company.cvm_code);
        self.add_alias(&company.legal_name);
        if let Some(name) = &company.trading_name {
            self.add_alias(name);
        }
    }

    pub fn matches_name(&self, candidate: &str) -> bool {
        let candidate = canonical_name(candidate);
        if candidate.is_empty() {
            return false;
        }
        self.aliases
            .iter()
            .map(|alias| canonical_name(alias))
            .any(|alias| {
                if alias == candidate {
                    return true;
                }
                // Fuzzy matching is deliberately conservative: after removing
                // legal suffixes and common abbreviations, allow one edit per 12 chars.
                let distance = levenshtein(&alias, &candidate);
                distance <= (alias.len().max(candidate.len()) / 12).max(1)
            })
    }

    fn add_alias(&mut self, value: &str) {
        if !value.trim().is_empty()
            && !self
                .aliases
                .iter()
                .any(|old| canonical_name(old) == canonical_name(value))
        {
            self.aliases.push(value.to_owned());
        }
    }
}

pub fn canonical_name(value: &str) -> String {
    let upper = value.trim().to_uppercase().replace("BCO", " BANCO ");
    let mut output = String::new();
    for ch in upper.chars() {
        let ch = match ch {
            'À' | 'Á' | 'Â' | 'Ã' | 'Ä' => 'A',
            'Ç' => 'C',
            'È' | 'É' | 'Ê' | 'Ë' => 'E',
            'Ì' | 'Í' | 'Î' | 'Ï' => 'I',
            'Ñ' => 'N',
            'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 'O',
            'Ù' | 'Ú' | 'Û' | 'Ü' => 'U',
            _ => ch,
        };
        if ch.is_ascii_alphanumeric() {
            output.push(ch);
        } else {
            output.push(' ');
        }
    }
    output
        .split_whitespace()
        .filter(|part| !matches!(*part, "SA" | "S" | "A" | "CIA" | "COMPANHIA" | "HOLDING"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn levenshtein(a: &str, b: &str) -> usize {
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.bytes().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.bytes().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (row[j + 1] + 1)
                .min(row[j] + 1)
                .min(diagonal + usize::from(ca != cb));
            diagonal = above;
        }
    }
    row[b.len()]
}

fn nonempty(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handles_banco_abbreviation_and_small_typos() {
        let issuer = B3Issuer {
            code: "123".into(),
            name: "BCO BRADESCO S.A.".into(),
            trading_name: "BRADESCO".into(),
        };
        let identity = CompanyIdentity::from_b3(&issuer);
        assert!(identity.matches_name("BANCO BRADESCO S.A."));
        assert!(identity.matches_name("BANCO BRADESCO"));
    }
}
