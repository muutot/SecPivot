use super::ImportCustomField;
use std::collections::{HashMap, HashSet};

/// External labels must remain data, never KeePass fields or plugin controls.
/// Unique names preserve every value after the KDBX field map is written.
pub(super) struct FieldNames {
    source: &'static str,
    next: HashMap<String, usize>,
    used: HashSet<String>,
}

impl FieldNames {
    pub(super) fn new(source: &'static str) -> Self {
        Self {
            source,
            next: HashMap::new(),
            used: HashSet::new(),
        }
    }

    pub(super) fn field(
        &mut self,
        label: &str,
        value: String,
        protected: bool,
    ) -> ImportCustomField {
        let label = label.trim();
        let base = format!(
            "{} {}",
            self.source,
            if label.is_empty() { "Field" } else { label }
        );
        let mut name = base.clone();
        let suffix = self.next.entry(base.clone()).or_insert(2);
        while !self.used.insert(name.clone()) {
            name = format!("{base} ({suffix})");
            *suffix += 1;
        }
        ImportCustomField {
            name,
            value,
            protected,
        }
    }
}
