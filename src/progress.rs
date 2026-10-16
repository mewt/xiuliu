use crate::{
    course::{self, Word},
    storage,
};
use chrono::{Days, NaiveDate};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Write},
    path::Path,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Progress {
    pub version: u8,
    pub started_on: NaiveDate,
    pub learned: BTreeMap<String, Memory>,
    pub days: BTreeMap<NaiveDate, StudyDay>,
    pub daily: BTreeMap<NaiveDate, String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Memory {
    pub first_learned: NaiveDate,
    pub last_practiced: NaiveDate,
    pub due_on: NaiveDate,
    pub interval_days: u64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StudyDay {
    pub xp: u64,
    pub words: BTreeSet<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Legacy {
    started_on: NaiveDate,
    completed: BTreeMap<NaiveDate, usize>,
}

impl Progress {
    pub fn new(today: NaiveDate) -> Self {
        Self {
            version: 2,
            started_on: today,
            learned: BTreeMap::new(),
            days: BTreeMap::new(),
            daily: BTreeMap::new(),
        }
    }
    pub fn load(path: &Path, today: NaiveDate, words: &[Word]) -> io::Result<Self> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::new(today)),
            Err(e) => return Err(e),
        };
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
        let legacy = value.get("version").is_none();
        let progress = if legacy {
            let old: Legacy = serde_json::from_value(value).map_err(io::Error::other)?;
            Self::migrate(old, today)?
        } else {
            serde_json::from_value(value).map_err(io::Error::other)?
        };
        progress.validate(today, words)?;
        if legacy {
            let backup = path.with_file_name("progress.v1.backup.json");
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(backup)
            {
                Ok(mut file) => {
                    file.write_all(&bytes)?;
                    file.sync_all()?;
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
            progress.save(path)?;
        }
        Ok(progress)
    }
    fn migrate(old: Legacy, today: NaiveDate) -> io::Result<Self> {
        let starter = course::starter();
        let mut progress = Self::new(old.started_on);
        for (date, index) in old.completed {
            if date < old.started_on
                || date > today
                || index >= starter.len()
                || date.signed_duration_since(old.started_on).num_days() as usize % starter.len()
                    != index
            {
                return Err(io::Error::other(
                    "Invalid legacy progress. Existing data was not changed.",
                ));
            }
            let id = &starter[index].simplified;
            progress.complete(id, date);
            progress.daily.insert(date, id.clone());
        }
        // Keep the old daily word for the migration day, then advance by learning.
        let index = today
            .signed_duration_since(old.started_on)
            .num_days()
            .max(0) as usize
            % starter.len();
        progress
            .daily
            .entry(today)
            .or_insert_with(|| starter[index].simplified.clone());
        Ok(progress)
    }
    fn validate(&self, today: NaiveDate, words: &[Word]) -> io::Result<()> {
        let ids: BTreeSet<&str> = words.iter().map(|w| w.simplified.as_str()).collect();
        let valid_date = |d: NaiveDate| d >= self.started_on && d <= today;
        if self.version != 2
            || self.started_on > today
            || self
                .daily
                .iter()
                .any(|(d, id)| !valid_date(*d) || !ids.contains(id.as_str()))
            || self.days.iter().any(|(d, day)| {
                !valid_date(*d)
                    || day.words.iter().any(|id| !ids.contains(id.as_str()))
                    || day.xp > day.words.len() as u64 * 20
            })
            || self.learned.iter().any(|(id, m)| {
                !ids.contains(id.as_str())
                    || !valid_date(m.first_learned)
                    || !valid_date(m.last_practiced)
                    || m.first_learned > m.last_practiced
                    || m.due_on < m.last_practiced
                    || !(1..=30).contains(&m.interval_days)
            })
        {
            return Err(io::Error::other(
                "Progress contains invalid words, dates, or scores. Existing data was not changed.",
            ));
        }
        Ok(())
    }
    pub fn next_word(&self, words: &[Word]) -> Option<usize> {
        words
            .iter()
            .position(|w| !self.learned.contains_key(&w.simplified))
    }
    pub fn today_word(&mut self, today: NaiveDate, words: &[Word]) -> Option<usize> {
        if let Some(id) = self.daily.get(&today) {
            return words.iter().position(|w| &w.simplified == id);
        }
        let index = self.next_word(words)?;
        self.daily.insert(today, words[index].simplified.clone());
        Some(index)
    }
    pub fn review_words(&self, words: &[Word], today: NaiveDate) -> Vec<usize> {
        let mut items: Vec<_> = words
            .iter()
            .enumerate()
            .filter_map(|(i, w)| self.learned.get(&w.simplified).map(|m| (m.due_on, i)))
            .collect();
        items.sort_by_key(|(date, i)| {
            (
                self.days
                    .get(&today)
                    .is_some_and(|day| day.words.contains(&words[*i].simplified)),
                *date > today,
                *date,
                *i,
            )
        });
        items.into_iter().map(|(_, i)| i).collect()
    }
    pub fn due_count(&self, today: NaiveDate) -> usize {
        self.learned.values().filter(|m| m.due_on <= today).count()
    }
    pub fn xp(&self) -> u64 {
        self.days.values().map(|d| d.xp).sum()
    }
    pub fn today_xp(&self, today: NaiveDate) -> u64 {
        self.days.get(&today).map_or(0, |d| d.xp)
    }
    pub fn complete(&mut self, id: &str, today: NaiveDate) -> u64 {
        let day = self.days.entry(today).or_default();
        if !day.words.insert(id.to_owned()) {
            return 0;
        }
        let xp = if let Some(memory) = self.learned.get_mut(id) {
            if memory.due_on <= today {
                memory.interval_days = (memory.interval_days * 2).min(30);
                memory.due_on = today
                    .checked_add_days(Days::new(memory.interval_days))
                    .unwrap_or(today);
            }
            memory.last_practiced = today;
            5
        } else {
            self.learned.insert(
                id.to_owned(),
                Memory {
                    first_learned: today,
                    last_practiced: today,
                    due_on: today.checked_add_days(Days::new(1)).unwrap_or(today),
                    interval_days: 1,
                },
            );
            20
        };
        day.xp += xp;
        xp
    }
    pub fn streak(&self, today: NaiveDate) -> usize {
        let active = |d: &NaiveDate| self.days.get(d).is_some_and(|day| !day.words.is_empty());
        let mut day = if active(&today) {
            Some(today)
        } else {
            today.checked_sub_days(Days::new(1))
        };
        let mut count = 0;
        while let Some(date) = day {
            if !active(&date) {
                break;
            }
            count += 1;
            day = date.checked_sub_days(Days::new(1));
        }
        count
    }
    pub fn save(&self, path: &Path) -> io::Result<()> {
        storage::save_json(path, self)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn date(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }
    #[test]
    fn daily_assignment_stays_fixed_and_missed_days_do_not_skip_words() {
        let words = course::bundled();
        let mut p = Progress::new(date("2024-02-28"));
        assert_eq!(p.today_word(date("2024-02-28"), &words), Some(0));
        p.complete("你好", date("2024-02-28"));
        assert_eq!(p.today_word(date("2024-02-28"), &words), Some(0));
        assert_eq!(p.today_word(date("2024-03-05"), &words), Some(1));
    }
    #[test]
    fn rewards_are_idempotent_and_reviews_space_out() {
        let mut p = Progress::new(date("2026-10-10"));
        assert_eq!(p.complete("你好", date("2026-10-10")), 20);
        assert_eq!(p.complete("你好", date("2026-10-10")), 0);
        assert_eq!(p.complete("你好", date("2026-10-11")), 5);
        assert_eq!(p.learned["你好"].due_on, date("2026-10-13"));
        assert_eq!(p.complete("你好", date("2026-10-12")), 5);
        assert_eq!(p.learned["你好"].due_on, date("2026-10-13"));
        assert_eq!(p.xp(), 30);
        assert_eq!(p.streak(date("2026-10-13")), 3);
        assert_eq!(p.streak(date("2026-10-14")), 0);
    }
    #[test]
    fn old_thirty_word_cycles_migrate_to_stable_word_ids() {
        let old = Legacy {
            started_on: date("2026-09-01"),
            completed: BTreeMap::from([(date("2026-09-01"), 0), (date("2026-10-01"), 0)]),
        };
        let p = Progress::migrate(old, date("2026-10-01")).unwrap();
        assert_eq!(p.learned.len(), 1);
        assert_eq!(p.days.len(), 2);
        assert_eq!(p.daily[&date("2026-10-01")], "你好");
    }
}
