use crate::course::Word;
use std::collections::BTreeSet;

pub const ROUNDS: usize = 3;

#[derive(Debug)]
pub struct Lesson {
    pub word: usize,
    pub round: usize,
    pub hearts: u8,
    pub selected: usize,
    pub choices: Vec<usize>,
    pub mistakes: u32,
    seed: u64,
}
impl Lesson {
    pub fn new(word: usize, words: &[Word], seed: u64, traditional: bool) -> Self {
        let mut lesson = Self {
            word,
            round: 0,
            hearts: 3,
            selected: 0,
            choices: Vec::new(),
            mistakes: 0,
            seed,
        };
        lesson.refresh(words, traditional);
        lesson
    }
    pub fn label<'a>(&self, word: &'a Word, traditional: bool) -> &'a str {
        match self.round {
            0 => word.meaning.split(';').next().unwrap_or(&word.meaning),
            1 => word.characters(traditional),
            _ => &word.pinyin,
        }
    }
    pub fn refresh(&mut self, words: &[Word], traditional: bool) {
        let target = &words[self.word];
        let normalize = |s: &str| s.to_lowercase().split_whitespace().collect::<String>();
        let mut seen = BTreeSet::from([normalize(self.label(target, traditional))]);
        let glosses: BTreeSet<_> = target
            .meaning
            .split(';')
            .map(|s| s.trim().to_lowercase())
            .collect();
        let mut choices = vec![self.word];
        let start = self.random() as usize % words.len();
        for offset in 0..words.len() {
            let index = (start + offset) % words.len();
            let word = &words[index];
            if word.level > target.level
                || index == self.word
                || (self.round < 2
                    && word
                        .meaning
                        .split(';')
                        .any(|s| glosses.contains(&s.trim().to_lowercase())))
            {
                continue;
            }
            if seen.insert(normalize(self.label(word, traditional))) {
                choices.push(index);
            }
            if choices.len() == 4 {
                break;
            }
        }
        for i in (1..choices.len()).rev() {
            let j = self.random() as usize % (i + 1);
            choices.swap(i, j);
        }
        self.choices = choices;
        self.selected = 0;
    }
    fn random(&mut self) -> u64 {
        self.seed = self
            .seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.seed ^ (self.seed >> 32)
    }
    pub fn answer(&mut self) -> bool {
        let correct = self.choices[self.selected] == self.word;
        if !correct {
            self.hearts = self.hearts.saturating_sub(1);
            self.mistakes += 1;
        }
        correct
    }
    pub fn advance(&mut self, words: &[Word], traditional: bool) -> bool {
        self.round += 1;
        if self.round == ROUNDS {
            true
        } else {
            self.refresh(words, traditional);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_round_has_one_correct_distinct_answer_and_three_correct_answers_finish() {
        let words = crate::course::bundled();
        for target in [0, 3, 30, words.len() - 1] {
            let mut lesson = Lesson::new(target, &words, 42, false);
            for round in 0..ROUNDS {
                assert_eq!(lesson.choices.len(), 4);
                assert_eq!(lesson.choices.iter().filter(|&&i| i == target).count(), 1);
                let labels: BTreeSet<_> = lesson
                    .choices
                    .iter()
                    .map(|&i| lesson.label(&words[i], false))
                    .collect();
                assert_eq!(labels.len(), 4);
                lesson.selected = lesson.choices.iter().position(|&i| i == target).unwrap();
                assert!(lesson.answer());
                assert_eq!(lesson.advance(&words, false), round == ROUNDS - 1);
            }
            assert_eq!(lesson.hearts, 3);
        }
    }
    #[test]
    fn errors_cost_hearts_but_do_not_advance_the_round() {
        let words = crate::course::bundled();
        let mut lesson = Lesson::new(0, &words, 9, false);
        for remaining in [2, 1, 0] {
            lesson.selected = lesson.choices.iter().position(|&i| i != 0).unwrap();
            assert!(!lesson.answer());
            assert_eq!(lesson.hearts, remaining);
            assert_eq!(lesson.round, 0);
            lesson.refresh(&words, false);
        }
    }
}
