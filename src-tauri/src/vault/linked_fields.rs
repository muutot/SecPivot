//! Bounded same-entry bindings. Source text is literal, never a macro or reference.
use super::{CustomField, FIELD_LINKED_FIELDS, FIELD_PASSWORD, FIELD_USERNAME, RESERVED_FIELDS};
use keepass::db::{EntryMut, EntryRef, Value};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use zeroize::Zeroizing;

const INVALID: &str = "链接字段无效；未应用任何链接";
const MAX_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum LinkedFieldTarget {
    Field { name: String },
    BitwardenFullName { fields: [String; 4] },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkedField {
    pub name: String,
    pub target: LinkedFieldTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkedFieldSet {
    pub version: u8,
    pub links: Vec<LinkedField>,
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name == name.trim()
        && name == js_trim(name)
        && name.len() <= 1024
        && name.chars().all(|c| {
            matches!(c as u32, 0x9 | 0xa | 0xd | 0x20..=0xd7ff | 0xe000..=0xfffd | 0x10000..=0x10ffff)
        })
}

impl LinkedFieldSet {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 || self.links.len() > 4096 {
            return Err(INVALID.into());
        }
        let mut aliases = HashSet::new();
        for link in &self.links {
            if let LinkedFieldTarget::BitwardenFullName { fields } = &link.target {
                if fields.iter().collect::<HashSet<_>>().len() != 4 {
                    return Err(INVALID.into());
                }
            }
            if !valid_name(&link.name)
                || RESERVED_FIELDS.contains(&link.name.as_str())
                || !aliases.insert(link.name.as_str())
            {
                return Err(INVALID.into());
            }
        }
        for link in &self.links {
            let names = match &link.target {
                LinkedFieldTarget::Field { name } => std::slice::from_ref(name),
                LinkedFieldTarget::BitwardenFullName { fields } => fields.as_slice(),
            };
            for name in names {
                if !valid_name(name)
                    || aliases.contains(name.as_str())
                    || (RESERVED_FIELDS.contains(&name.as_str())
                        && !matches!(name.as_str(), FIELD_USERNAME | FIELD_PASSWORD))
                    || (matches!(link.target, LinkedFieldTarget::BitwardenFullName { .. })
                        && RESERVED_FIELDS.contains(&name.as_str()))
                {
                    return Err(INVALID.into());
                }
            }
        }
        let encoded = Zeroizing::new(serde_json::to_string(self).map_err(|_| INVALID)?);
        if encoded.len() > MAX_BYTES {
            return Err(INVALID.into());
        }
        Ok(())
    }

    pub fn from_json(raw: &str) -> Result<Self, String> {
        if raw.len() > MAX_BYTES {
            return Err(INVALID.into());
        }
        let value: Self = serde_json::from_str(raw).map_err(|_| INVALID)?;
        value.validate()?;
        Ok(value)
    }

    pub fn as_json(&self) -> Result<Zeroizing<String>, String> {
        self.validate()?;
        Ok(Zeroizing::new(
            serde_json::to_string(self).map_err(|_| INVALID)?,
        ))
    }

    pub(crate) fn validate_input(&self, fields: &[CustomField]) -> Result<(), String> {
        self.validate()?;
        for link in &self.links {
            if fields.iter().filter(|f| f.name.trim() == link.name).count() != 1
                || !fields
                    .iter()
                    .any(|f| f.name.trim() == link.name && f.protected)
            {
                return Err(INVALID.into());
            }
        }
        Ok(())
    }
}

pub(super) fn read(entry: &EntryRef<'_>) -> Result<Option<LinkedFieldSet>, String> {
    entry
        .get(FIELD_LINKED_FIELDS)
        .map(LinkedFieldSet::from_json)
        .transpose()
}

fn js_trim(value: &str) -> &str {
    value.trim_matches(|ch: char| (ch.is_whitespace() && ch != '\u{0085}') || ch == '\u{feff}')
}

fn resolve(entry: &EntryRef<'_>, target: &LinkedFieldTarget) -> Zeroizing<String> {
    Zeroizing::new(match target {
        LinkedFieldTarget::Field { name } => entry.get(name).unwrap_or_default().to_owned(),
        LinkedFieldTarget::BitwardenFullName { fields } => {
            let value = Zeroizing::new(
                fields
                    .iter()
                    .map(|name| entry.get(name).unwrap_or_default())
                    .filter(|part| !js_trim(part).is_empty())
                    .collect::<Vec<_>>()
                    .join(" "),
            );
            js_trim(&value).to_owned()
        }
    })
}

pub(super) fn value(entry: &EntryRef<'_>, name: &str) -> Result<Option<String>, String> {
    if entry.get(name).is_none() {
        return Ok(None);
    }
    let bindings = read(entry)?;
    if let Some(link) = bindings
        .as_ref()
        .and_then(|s| s.links.iter().find(|l| l.name == name))
    {
        let mut resolved = resolve(entry, &link.target);
        return Ok(Some(std::mem::take(&mut *resolved)));
    }
    Ok(entry.get(name).map(str::to_owned))
}

pub(super) fn input(fields: &[CustomField]) -> Result<Option<LinkedFieldSet>, String> {
    let mut found = None;
    for field in fields
        .iter()
        .filter(|f| f.name.trim() == FIELD_LINKED_FIELDS)
    {
        if found.is_some() {
            return Err(INVALID.into());
        }
        let set = LinkedFieldSet::from_json(&field.value)?;
        if !field.protected {
            return Err(INVALID.into());
        }
        set.validate_input(fields)?;
        found = Some(set);
    }
    Ok(found)
}

pub(super) fn set(entry: &mut EntryMut<'_>, bindings: Option<&LinkedFieldSet>) {
    match bindings.filter(|s| !s.links.is_empty()) {
        Some(bindings) => {
            if let Ok(mut raw) = bindings.as_json() {
                entry.set(
                    FIELD_LINKED_FIELDS,
                    Value::protected(std::mem::take(&mut *raw)),
                );
            }
        }
        None => {
            entry.fields.remove(FIELD_LINKED_FIELDS);
        }
    }
    refresh(entry);
}

/// Called inside the caller's existing history transaction, never on a snapshot read.
/// Invalid foreign metadata cannot cause any evaluation or automatic field creation.
pub(super) fn refresh(entry: &mut EntryMut<'_>) {
    let Ok(Some(mut bindings)) = read(&entry.as_ref()) else {
        return;
    };
    bindings
        .links
        .retain(|link| entry.fields.contains_key(&link.name));
    let values = bindings
        .links
        .iter()
        .map(|link| (link.name.clone(), resolve(&entry.as_ref(), &link.target)))
        .collect::<Vec<_>>();
    for (name, mut value) in values {
        entry.set(name, Value::protected(std::mem::take(&mut *value)));
    }
    if bindings.links.is_empty() {
        entry.fields.remove(FIELD_LINKED_FIELDS);
    } else if let Ok(mut raw) = bindings.as_json() {
        entry.set(
            FIELD_LINKED_FIELDS,
            Value::protected(std::mem::take(&mut *raw)),
        );
    }
}

/// Preserve unchanged aliases; explicit literal edits, unprotection and deletion detach.
pub(super) fn retain_for_fields(
    entry: &EntryRef<'_>,
    desired: &[CustomField],
) -> Option<LinkedFieldSet> {
    let Ok(Some(mut bindings)) = read(entry) else {
        return None;
    };
    let fields: HashMap<_, _> = desired.iter().map(|f| (f.name.trim(), f)).collect();
    bindings.links.retain(|link| {
        fields.get(link.name.as_str()).is_some_and(|field| {
            field.protected
                && (entry.get(&link.name) == Some(field.value.as_str())
                    || resolve(entry, &link.target).as_str() == field.value)
        })
    });
    Some(bindings)
}

pub(super) fn detach(entry: &mut EntryMut<'_>, name: &str) -> Result<(), String> {
    if let Some(mut bindings) = read(&entry.as_ref())? {
        bindings.links.retain(|link| link.name != name);
        set(entry, Some(&bindings));
    }
    Ok(())
}

#[cfg(test)]
#[path = "linked_fields_tests.rs"]
mod tests;
