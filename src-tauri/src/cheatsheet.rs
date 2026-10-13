//! The cheat sheet: built-in content, optional user overrides, and prefix substitution.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;

const BUILTIN: &str = include_str!("cheatsheet.json");

/// Placeholder in `keys` that is replaced by the user's tmux prefix.
const PREFIX_TOKEN: &str = "Prefix";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cheatsheet {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    pub sections: Vec<Section>,
    /// In a user file: replace the built-in sheet instead of merging into it.
    #[serde(default, skip_serializing)]
    pub replace: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub keys: String,
    pub desc: String,
}

pub fn builtin() -> Cheatsheet {
    serde_json::from_str(BUILTIN).expect("built-in cheatsheet.json is valid")
}

/// Merges a user sheet into `base`: sections with the same name (ignoring case) get the
/// user's items appended, new sections are added at the end. `replace: true` wins outright.
pub fn merge(mut base: Cheatsheet, user: Cheatsheet) -> Cheatsheet {
    if user.replace {
        return user;
    }
    for section in user.sections {
        match base
            .sections
            .iter_mut()
            .find(|s| s.name.eq_ignore_ascii_case(&section.name))
        {
            Some(existing) => existing.items.extend(section.items),
            None => base.sections.push(section),
        }
    }
    base
}

/// Replaces the `Prefix` placeholder in every binding with `prefix` (e.g. `Ctrl+a`).
pub fn with_prefix(mut sheet: Cheatsheet, prefix: &str) -> Cheatsheet {
    for item in sheet.sections.iter_mut().flat_map(|s| s.items.iter_mut()) {
        item.keys = substitute(&item.keys, prefix);
    }
    sheet
}

fn substitute(keys: &str, prefix: &str) -> String {
    keys.split(' ')
        .map(|token| if token == PREFIX_TOKEN { prefix } else { token })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Built-in sheet merged with the user's `cheatsheet.json`, if any. A broken user file
/// does not hide the cheat sheet: the built-in one is returned with a warning.
pub fn load() -> (Cheatsheet, Option<String>) {
    let path = crate::settings::cheatsheet_path();
    let user = match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str::<Cheatsheet>(&text)
            .map_err(|e| format!("ignored {}: {e}", path.display())),
        Err(e) if e.kind() == ErrorKind::NotFound => return (builtin(), None),
        Err(e) => Err(format!("ignored {}: {e}", path.display())),
    };
    match user {
        Ok(user) => (merge(builtin(), user), None),
        Err(warning) => (builtin(), Some(warning)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet(json: &str) -> Cheatsheet {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn builtin_parses_and_uses_the_prefix_token() {
        let b = builtin();
        assert!(b.sections.len() >= 5);
        assert!(
            b.sections
                .iter()
                .flat_map(|s| &s.items)
                .any(|i| i.keys.starts_with("Prefix "))
        );
    }

    #[test]
    fn prefix_is_substituted_as_a_whole_word_only() {
        assert_eq!(substitute("Prefix d", "Ctrl+a"), "Ctrl+a d");
        assert_eq!(substitute("Prefix ( / )", "C-a"), "C-a ( / )");
        assert_eq!(substitute("tmux a -t name", "Ctrl+a"), "tmux a -t name");
        assert_eq!(substitute("Prefixes", "Ctrl+a"), "Prefixes");
    }

    #[test]
    fn user_sections_merge_by_name_and_append() {
        let base = sheet(
            r#"{ "sections": [ { "name": "Panes", "items": [ { "keys": "a", "desc": "A" } ] } ] }"#,
        );
        let user = sheet(
            r#"{ "sections": [
                { "name": "panes", "items": [ { "keys": "b", "desc": "B" } ] },
                { "name": "Mine", "items": [ { "keys": "c", "desc": "C" } ] } ] }"#,
        );
        let merged = merge(base, user);
        assert_eq!(merged.sections.len(), 2);
        assert_eq!(merged.sections[0].items.len(), 2);
        assert_eq!(merged.sections[1].name, "Mine");
    }

    #[test]
    fn replace_discards_the_builtin_sheet() {
        let user = sheet(r#"{ "replace": true, "sections": [] }"#);
        assert!(merge(builtin(), user).sections.is_empty());
    }
}
