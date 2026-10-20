use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io, ops::Range, path::Path};

#[derive(Clone, Debug, Deserialize)]
struct Chapter {
    id: String,
    title: String,
    words: Vec<String>,
}

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
fn chapters() -> Vec<Chapter> {
    let chapters: Vec<Chapter> =
        serde_json::from_str(include_str!("../assets/course.json")).expect("valid course manifest");
    for (index, chapter) in chapters.iter().enumerate() {
        assert_eq!(chapter.id, format!("lesson-{:02}", index + 1));
    }
    chapters
}
pub fn bundled() -> Vec<Word> {
    let dictionary: Vec<Word> =
        serde_json::from_str(include_str!("../assets/hsk.json")).expect("valid HSK dictionary");
    let textbook: Vec<Word> = serde_json::from_str(include_str!("../assets/textbook-words.json"))
        .expect("valid textbook vocabulary");
    let mut available: BTreeMap<_, _> = dictionary
        .into_iter()
        .map(|word| (word.simplified.clone(), word))
        .collect();
    for word in starter().into_iter().chain(textbook) {
        available.insert(word.simplified.clone(), word);
    }
    let words = chapters()
        .into_iter()
        .flat_map(|chapter| chapter.words)
        .map(|id| {
            available
                .get(&id)
                .unwrap_or_else(|| panic!("course word {id} is missing from the dictionary"))
                .clone()
        })
        .collect();
    add_examples(words)
}
pub fn chapter(index: usize) -> usize {
    chapters()
        .iter()
        .scan(0, |end, chapter| {
            *end += chapter.words.len();
            Some(*end)
        })
        .position(|end| index < end)
        .map_or_else(chapter_count, |chapter| chapter + 1)
}
pub fn chapter_count() -> usize {
    chapters().len()
}
pub fn chapter_progress(index: usize) -> (usize, usize) {
    let chapters = chapters();
    let mut start = 0;
    for chapter in chapters {
        let end = start + chapter.words.len();
        if index < end {
            return (index - start + 1, chapter.words.len());
        }
        start = end;
    }
    (0, 0)
}
pub fn chapter_range(chapter: usize) -> Range<usize> {
    let chapters = chapters();
    let start = chapters
        .iter()
        .take(chapter.saturating_sub(1))
        .map(|chapter| chapter.words.len())
        .sum();
    let size = chapters
        .get(chapter.saturating_sub(1))
        .map_or(0, |chapter| chapter.words.len());
    start..start + size
}
pub fn chapter_title(chapter: usize) -> String {
    chapters()
        .get(chapter.saturating_sub(1))
        .map_or_else(String::new, |chapter| chapter.title.clone())
}
fn add_examples(mut words: Vec<Word>) -> Vec<Word> {
    let examples: Vec<Word> = serde_json::from_str(include_str!("../assets/examples.json"))
        .expect("valid bundled examples");
    let positions: BTreeMap<_, _> = words
        .iter()
        .enumerate()
        .map(|(index, word)| (word.simplified.clone(), index))
        .collect();
    for example in examples {
        if let Some(&index) = positions.get(&example.simplified) {
            words[index].example = example.example;
            words[index].example_traditional = example.example_traditional;
            words[index].example_pinyin = example.example_pinyin;
            words[index].translation = example.translation;
        }
    }
    words
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
pub fn load(_directory: &Path) -> io::Result<Vec<Word>> {
    let words = bundled();
    validate(&words)?;
    Ok(words)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_course_follows_the_explicit_textbook_lessons() {
        let words = bundled();
        validate(&words).unwrap();
        assert_eq!(words.len(), 166);
        assert!(words.iter().all(|word| word.level == 1));
        assert_eq!(chapter_count(), 15);
        assert_eq!(chapter_title(1), "你好");
        assert_eq!(chapter_range(1), 0..6);
        assert_eq!(chapter_range(15), 157..166);
        assert_eq!(chapter(0), 1);
        assert_eq!(chapter(5), 1);
        assert_eq!(chapter(6), 2);
        assert_eq!(chapter_progress(165), (9, 9));
        assert_eq!(words[0].simplified, "你");
        assert_eq!(words[6].simplified, "谢谢");
        let location = words.iter().find(|word| word.simplified == "在").unwrap();
        assert_eq!(location.example, "我的朋友在北京。");
        assert_eq!(location.example_pinyin, "Wǒ de péngyou zài Běijīng.");
        assert_eq!(location.translation, "My friend is in Beijing.");
        // Every course word now ships with an example sentence.
        assert!(words.iter().all(|word| !word.example.is_empty()));
    }
}

#[cfg(test)]
mod verify_fixes {
    use super::*;
    #[test]
    fn corrected_words_resolve_to_the_beginner_sense() {
        let words = bundled();
        let get = |s: &str| words.iter().find(|w| w.simplified == s).unwrap();
        assert_eq!(get("读").pinyin, "dú");
        assert_eq!(get("个").pinyin, "gè");
        assert_eq!(get("那").pinyin, "nà");
        assert_eq!(get("吗").pinyin, "ma");
        assert_eq!(get("吧").pinyin, "ba");
        assert_eq!(get("几").pinyin, "jǐ");
        assert!(get("读").meaning.contains("read"));
        assert!(get("读").example.contains('读'));
    }
}
