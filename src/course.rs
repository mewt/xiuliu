use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read},
    path::Path,
    time::Duration,
};

pub const SOURCE: &str =
    "https://raw.githubusercontent.com/drkameleon/complete-hsk-vocabulary/main/complete.json";
const MAX_DOWNLOAD: u64 = 20_000_000;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Word {
    pub simplified: String,
    pub traditional: String,
    pub pinyin: String,
    pub meaning: String,
    #[serde(default = "beginner")]
    pub level: u8,
    #[serde(default)]
    pub frequency: u32,
    #[serde(default)]
    pub example: String,
    #[serde(default)]
    pub example_traditional: String,
    #[serde(default)]
    pub example_pinyin: String,
    #[serde(default)]
    pub translation: String,
}
fn beginner() -> u8 {
    1
}
impl Word {
    pub fn characters(&self, traditional: bool) -> &str {
        if traditional {
            &self.traditional
        } else {
            &self.simplified
        }
    }
    pub fn sentence(&self, traditional: bool) -> &str {
        if traditional {
            &self.example_traditional
        } else {
            &self.example
        }
    }
}
pub fn starter() -> Vec<Word> {
    serde_json::from_str(include_str!("../assets/words.json")).expect("valid starter course")
}
pub fn bundled() -> Vec<Word> {
    let extra: Vec<Word> =
        serde_json::from_str(include_str!("../assets/hsk.json")).expect("valid HSK course");
    merge(starter(), extra)
}
// Keep existing order and identities. New entries append by level and frequency.
pub fn merge(mut existing: Vec<Word>, mut incoming: Vec<Word>) -> Vec<Word> {
    let mut positions: BTreeMap<String, usize> = existing
        .iter()
        .enumerate()
        .map(|(i, w)| (w.simplified.clone(), i))
        .collect();
    incoming.sort_by_key(|w| (w.level, w.frequency, w.simplified.clone()));
    for word in incoming {
        if let Some(&i) = positions.get(&word.simplified) {
            if existing[i].example.is_empty() {
                existing[i] = word;
            }
        } else {
            positions.insert(word.simplified.clone(), existing.len());
            existing.push(word);
        }
    }
    existing
}
fn validate(words: &[Word]) -> io::Result<()> {
    let mut seen = std::collections::BTreeSet::new();
    if words.is_empty() || words.len() > 30_000 {
        return Err(io::Error::other("Invalid vocabulary size"));
    }
    for word in words {
        if !seen.insert(&word.simplified)
            || !(1..=7).contains(&word.level)
            || [
                &word.simplified,
                &word.traditional,
                &word.pinyin,
                &word.meaning,
            ]
            .iter()
            .any(|s| s.trim().is_empty() || s.len() > 4000 || s.chars().any(char::is_control))
        {
            return Err(io::Error::other(
                "Vocabulary contains invalid or duplicate entries; existing data was not changed",
            ));
        }
    }
    Ok(())
}
pub fn load(directory: &Path) -> io::Result<Vec<Word>> {
    match fs::read(directory.join("vocabulary.json")) {
        Ok(bytes) => {
            let cached: Vec<Word> = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
            validate(&cached)?;
            // Cached updates take precedence over the older bundled snapshot.
            Ok(merge(bundled(), cached))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(bundled()),
        Err(e) => Err(e),
    }
}
#[derive(Deserialize)]
struct Entry {
    simplified: String,
    level: Vec<String>,
    #[serde(default)]
    frequency: Option<u32>,
    forms: Vec<Form>,
}
#[derive(Deserialize)]
struct Form {
    traditional: String,
    transcriptions: Transcriptions,
    meanings: Vec<String>,
}
#[derive(Deserialize)]
struct Transcriptions {
    pinyin: String,
}

pub fn parse_download(bytes: &[u8]) -> io::Result<Vec<Word>> {
    let entries: Vec<Entry> = serde_json::from_slice(bytes).map_err(io::Error::other)?;
    let mut words = Vec::new();
    for entry in entries {
        let level = entry
            .level
            .iter()
            .filter_map(|l| l.strip_prefix("new-").and_then(|n| n.parse::<u8>().ok()))
            .min()
            .or_else(|| {
                entry
                    .level
                    .iter()
                    .filter_map(|l| l.strip_prefix("old-").and_then(|n| n.parse::<u8>().ok()))
                    .min()
            });
        let Some(level) = level else {
            continue;
        };
        let form = entry
            .forms
            .iter()
            .find(|f| {
                !f.meanings.is_empty()
                    && !f.meanings.iter().all(|m| m.starts_with("surname "))
                    && f.transcriptions
                        .pinyin
                        .chars()
                        .next()
                        .is_some_and(char::is_lowercase)
            })
            .or_else(|| entry.forms.first());
        let Some(form) = form else {
            return Err(io::Error::other("Vocabulary entry has no pronunciation"));
        };
        words.push(Word {
            simplified: entry.simplified,
            traditional: form.traditional.clone(),
            pinyin: form.transcriptions.pinyin.clone(),
            meaning: form
                .meanings
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join("; "),
            level,
            frequency: entry.frequency.unwrap_or(u32::MAX),
            example: String::new(),
            example_traditional: String::new(),
            example_pinyin: String::new(),
            translation: String::new(),
        });
    }
    validate(&words)?;
    Ok(words)
}
pub fn update(directory: &Path, existing: Vec<Word>) -> io::Result<(usize, usize)> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(45))
        .build();
    let response = agent
        .get(SOURCE)
        .set("User-Agent", concat!("xiuliu/", env!("CARGO_PKG_VERSION")))
        .call()
        .map_err(|e| {
            io::Error::other(format!(
                "Could not download free HSK vocabulary: {e}. Your offline course is unchanged."
            ))
        })?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(MAX_DOWNLOAD + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_DOWNLOAD {
        return Err(io::Error::other("Vocabulary download exceeded 20 MB"));
    }
    install_download(directory, existing, &bytes)
}

pub fn install_download(
    directory: &Path,
    existing: Vec<Word>,
    bytes: &[u8],
) -> io::Result<(usize, usize)> {
    if bytes.len() as u64 > MAX_DOWNLOAD {
        return Err(io::Error::other("Vocabulary file exceeded 20 MB"));
    }
    let incoming = parse_download(bytes)?;
    if incoming.len() < 1000 {
        return Err(io::Error::other(
            "Downloaded course looks incomplete; keeping existing vocabulary",
        ));
    }
    let old_count = existing.len();
    let merged = merge(existing, incoming);
    crate::storage::save_json(&directory.join("vocabulary.json"), &merged)?;
    Ok((merged.len(), merged.len() - old_count))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_course_is_complete_and_preserves_starter_order() {
        let words = bundled();
        validate(&words).unwrap();
        assert!(words.len() > 450);
        assert_eq!(words[0].simplified, "你好");
        assert_eq!(words[29].simplified, "再见");
    }
    #[test]
    fn updates_do_not_remove_words_or_change_starter_examples() {
        let words = bundled();
        let mut incoming = words.clone();
        incoming.reverse();
        let merged = merge(words.clone(), incoming);
        assert_eq!(
            words.iter().map(|w| &w.simplified).collect::<Vec<_>>(),
            merged.iter().map(|w| &w.simplified).collect::<Vec<_>>()
        );
        assert_eq!(merged[0].example, "你好！");
        let mut archaic = words
            .iter()
            .find(|word| word.simplified == "听")
            .unwrap()
            .clone();
        archaic.pinyin = "yǐn".into();
        archaic.meaning = "smile (archaic)".into();
        archaic.example.clear();
        let merged = merge(words, vec![archaic]);
        let listening = merged.iter().find(|word| word.simplified == "听").unwrap();
        assert_eq!(listening.pinyin, "tīng");
        assert_eq!(listening.meaning, "to listen");
    }
    #[test]
    fn malformed_downloads_are_rejected() {
        assert!(parse_download(b"not json").is_err());
        assert!(parse_download(b"[]").is_err());
    }
}
