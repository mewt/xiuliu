use crate::{
    analytics,
    audio::{Audio, Cue},
    course::Word,
    lesson::{Lesson, ROUNDS},
    progress::Progress,
};
use chrono::Local;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Gauge, Paragraph, Wrap},
};
use std::{
    io,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub(super) const BG: Color = Color::Rgb(17, 23, 35);
pub(super) const PANEL: Color = Color::Rgb(26, 35, 49);
pub(super) const GREEN: Color = Color::Rgb(167, 231, 88);
pub(super) const WHITE: Color = Color::Rgb(236, 242, 249);
pub(super) const MUTED: Color = Color::Rgb(149, 166, 187);
pub(super) const PURPLE: Color = Color::Rgb(191, 166, 255);
const RED: Color = Color::Rgb(255, 143, 151);
pub(super) const GOLD: Color = Color::Rgb(255, 211, 106);

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Home,
    Analytics,
    Study,
    Question,
    Detail,
    Feedback(bool),
    Complete,
    Retry,
    Empty,
    Finished,
}

struct App<'a> {
    words: &'a [Word],
    progress: &'a mut Progress,
    path: &'a Path,
    traditional: bool,
    screen: Screen,
    lesson: Option<Lesson>,
    review: bool,
    earned: u64,
    scroll: u16,
    audio: Audio,
    analytics: analytics::State,
}
impl App<'_> {
    fn start(&mut self, bonus: bool, review: bool) -> io::Result<()> {
        self.audio.stop();
        let today = Local::now().date_naive();
        self.review = review;
        let index = if review {
            let queue = self.progress.review_words(self.words, today);
            queue.first().copied()
        } else if bonus {
            self.progress.next_word(self.words)
        } else {
            self.progress.today_word(today, self.words)
        };
        if let Some(index) = index {
            let seed = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;
            self.lesson = Some(Lesson::new(index, self.words, seed, self.traditional));
            self.screen = if review {
                Screen::Question
            } else {
                Screen::Study
            };
            self.scroll = 0;
            self.progress.save(self.path)?;
            if !review {
                self.audio.speak(
                    self.words[index].characters(self.traditional),
                    &self.words[index].pinyin,
                    false,
                );
            }
        } else {
            self.screen = if review {
                Screen::Empty
            } else {
                Screen::Finished
            };
        }
        Ok(())
    }
    fn handle(&mut self, code: KeyCode) -> io::Result<()> {
        if self.screen == Screen::Analytics {
            if matches!(code, KeyCode::Esc | KeyCode::Char('h' | 'g')) {
                self.screen = Screen::Home;
            } else {
                self.analytics.handle(code);
            }
            return Ok(());
        }
        if code == KeyCode::Char('g')
            && matches!(
                self.screen,
                Screen::Home | Screen::Complete | Screen::Finished | Screen::Empty
            )
        {
            self.audio.stop();
            self.screen = Screen::Analytics;
            return Ok(());
        }
        if code == KeyCode::Char('m') {
            self.audio.toggle();
            return Ok(());
        }
        if matches!(code, KeyCode::Char('s' | 'S' | 'a')) {
            if matches!(
                self.screen,
                Screen::Study
                    | Screen::Question
                    | Screen::Feedback(_)
                    | Screen::Complete
                    | Screen::Retry
            ) {
                if let Some(lesson) = &self.lesson {
                    let word = &self.words[lesson.word];
                    let text = if code == KeyCode::Char('a') {
                        word.sentence(self.traditional)
                    } else {
                        word.characters(self.traditional)
                    };
                    let pinyin = if code == KeyCode::Char('a') {
                        &word.example_pinyin
                    } else {
                        &word.pinyin
                    };
                    self.audio.speak(text, pinyin, code == KeyCode::Char('S'));
                }
            } else {
                self.audio.message("Open a lesson to hear a word · m mute");
            }
            return Ok(());
        }
        if self.screen == Screen::Detail {
            match code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('?') => {
                    self.screen = Screen::Question;
                    self.scroll = 0;
                }
                KeyCode::Down => self.scroll = self.scroll.saturating_add(1).min(100),
                KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
                _ => {}
            }
            return Ok(());
        }
        if code == KeyCode::Esc || code == KeyCode::Char('h') {
            self.audio.stop();
            self.screen = Screen::Home;
            self.scroll = 0;
            return Ok(());
        }
        if code == KeyCode::Char('r')
            && matches!(
                self.screen,
                Screen::Home | Screen::Complete | Screen::Finished | Screen::Empty
            )
        {
            return self.start(false, true);
        }
        match self.screen {
            Screen::Home if code == KeyCode::Enter => {
                let today = Local::now().date_naive();
                let done = self
                    .progress
                    .today_word(today, self.words)
                    .is_some_and(|i| {
                        self.progress
                            .learned
                            .contains_key(&self.words[i].simplified)
                    });
                self.start(done, false)?;
            }
            Screen::Study => match code {
                KeyCode::Enter => {
                    self.audio.stop();
                    self.screen = Screen::Question;
                    self.scroll = 0;
                }
                KeyCode::Down => self.scroll = self.scroll.saturating_add(1).min(40),
                KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
                _ => {}
            },
            Screen::Question => {
                let lesson = self.lesson.as_mut().unwrap();
                let count = lesson.choices.len();
                match code {
                    KeyCode::Char(c @ '1'..='4') if (c as usize - '1' as usize) < count => {
                        lesson.selected = c as usize - '1' as usize
                    }
                    KeyCode::Down | KeyCode::Right | KeyCode::Tab => {
                        lesson.selected = (lesson.selected + 1) % count
                    }
                    KeyCode::Up | KeyCode::Left => {
                        lesson.selected = (lesson.selected + count - 1) % count
                    }
                    KeyCode::Char('?') => {
                        self.screen = Screen::Detail;
                        self.scroll = 0;
                    }
                    KeyCode::Enter => {
                        let correct = lesson.answer();
                        self.screen = Screen::Feedback(correct);
                        self.audio
                            .cue(if correct { Cue::Correct } else { Cue::TryAgain });
                    }
                    _ => {}
                }
            }
            Screen::Feedback(correct) if code == KeyCode::Enter => {
                let lesson = self.lesson.as_mut().unwrap();
                if lesson.hearts == 0 {
                    self.screen = Screen::Retry;
                } else if correct {
                    if lesson.advance(self.words, self.traditional) {
                        let mut updated = self.progress.clone();
                        self.earned = updated.complete(
                            &self.words[lesson.word].simplified,
                            Local::now().date_naive(),
                        );
                        updated.save(self.path)?;
                        *self.progress = updated;
                        self.screen = Screen::Complete;
                        self.audio.cue(Cue::Complete);
                    } else {
                        self.screen = Screen::Question;
                    }
                } else {
                    lesson.refresh(self.words, self.traditional);
                    self.screen = Screen::Question;
                }
            }
            Screen::Complete if code == KeyCode::Enter || code == KeyCode::Char('n') => {
                self.start(true, false)?;
            }
            Screen::Retry if code == KeyCode::Enter => {
                let index = self.lesson.as_ref().unwrap().word;
                self.lesson = Some(Lesson::new(
                    index,
                    self.words,
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos() as u64,
                    self.traditional,
                ));
                self.screen = Screen::Study;
                self.scroll = 0;
                self.audio.speak(
                    self.words[index].characters(self.traditional),
                    &self.words[index].pinyin,
                    false,
                );
            }
            Screen::Empty | Screen::Finished if code == KeyCode::Enter => {
                self.screen = Screen::Home
            }
            _ => {}
        }
        Ok(())
    }
    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        frame.render_widget(
            Block::default().style(Style::default().bg(BG).fg(WHITE)),
            area,
        );
        if area.width < 60 || area.height < 26 {
            text(
                frame,
                area,
                "Make a little room to grow.\nResize to at least 60 × 26.\nPress q to quit.",
                WHITE,
                Alignment::Center,
            );
            return;
        }
        if self.screen == Screen::Analytics {
            analytics::render(
                frame,
                area,
                self.words,
                self.progress,
                &self.analytics,
                Local::now().date_naive(),
            );
            return;
        }
        let width = area.width.min(100);
        let root = Rect::new((area.width - width) / 2 + 2, 1, width - 4, area.height - 2);
        let rows = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(14),
            Constraint::Length(3),
        ])
        .split(root);
        let head = Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(rows[0]);
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(vec![
                    Span::styled("✦ XIULIU", Style::default().fg(GREEN).bold()),
                    Span::styled("   一天一词", Style::default().fg(WHITE)),
                ]),
                Line::from(Span::styled(
                    "Small lessons. Real progress.",
                    Style::default().fg(MUTED),
                )),
            ]),
            head[0],
        );
        text(
            frame,
            head[1],
            &format!(
                "{}\n{}",
                Local::now().format("%a, %d %b"),
                if self.traditional {
                    "TRADITIONAL"
                } else {
                    "SIMPLIFIED"
                }
            ),
            MUTED,
            Alignment::Right,
        );
        let today = Local::now().date_naive();
        let stats = Layout::horizontal([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(rows[1]);
        text(
            frame,
            stats[0],
            &format!("✦ {} XP", self.progress.xp()),
            GOLD,
            Alignment::Left,
        );
        text(
            frame,
            stats[1],
            &format!("{} DAY STREAK", self.progress.streak(today)),
            PURPLE,
            Alignment::Center,
        );
        text(
            frame,
            stats[2],
            &format!(
                "{} / {} WORDS",
                self.progress.learned.len(),
                self.words.len()
            ),
            GREEN,
            Alignment::Right,
        );
        match self.screen {
            Screen::Home => self.home(frame, rows[2]),
            Screen::Analytics => unreachable!("analytics renders above"),
            Screen::Study => self.study(frame, rows[2]),
            Screen::Question | Screen::Feedback(_) => self.question(frame, rows[2]),
            Screen::Detail => {
                let lesson = self.lesson.as_ref().unwrap();
                let choice = &self.words[lesson.choices[lesson.selected]];
                frame.render_widget(Paragraph::new(lesson.label(choice, self.traditional)).wrap(Wrap { trim: true })
                    .scroll((self.scroll, 0)).block(panel(" FULL ANSWER OPTION · Enter to return ", PURPLE)), rows[2]);
            }
            Screen::Complete => self.complete(frame, rows[2]),
            Screen::Retry => self.message(frame, rows[2], "A fresh start?", "Mistakes are part of learning.\nTake another look at the word, then try again.\nYour progress is safe. Your hearts refill for free.", "TRY AGAIN  [Enter]", RED),
            Screen::Empty => self.message(frame, rows[2], "Your collection starts here.", "Finish your first lesson to unlock review.\nA little practice today goes a long way.", "BACK TO YOUR PATH  [Enter]", GREEN),
            Screen::Finished => self.message(frame, rows[2], "Look how far you have grown!", "You have learned every word in your course.\nPress r to review and keep your memory fresh.\nRun xiuliu update for the latest free vocabulary.", "BACK TO YOUR PATH  [Enter]", GREEN),
        }
        let footer = match self.screen {
            Screen::Home => "Enter start · r review · g analytics · q quit",
            Screen::Study => "Enter start challenge     ↑↓ read more     Esc home     q quit",
            Screen::Question => "1–4 choose · Enter check · ? full option · Esc home · q quit",
            Screen::Detail => "↑↓ scroll     Enter / Esc back to challenge     q quit",
            Screen::Feedback(_) => "Enter continue     Esc home     q quit",
            Screen::Complete => "Enter more · r review · g analytics · Esc home · q quit",
            _ => "Enter continue     Esc home     q quit",
        };
        text(
            frame,
            rows[3],
            &format!("{}\n{footer}", self.audio.status()),
            MUTED,
            Alignment::Center,
        );
    }
    fn home(&mut self, frame: &mut Frame, area: Rect) {
        let today = Local::now().date_naive();
        let done = self
            .progress
            .today_word(today, self.words)
            .is_some_and(|i| {
                self.progress
                    .learned
                    .contains_key(&self.words[i].simplified)
            });
        let parts = Layout::vertical([
            Constraint::Min(7),
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(area);
        let next = self.progress.next_word(self.words);
        let level = next.map_or(1, |i| self.words[i].level);
        let hero_block = panel(" YOUR DAILY PATH ", PURPLE);
        let inner = hero_block.inner(parts[0]);
        frame.render_widget(hero_block, parts[0]);
        let content_height = inner.height.min(8);
        let lines = if content_height < 8 {
            vec![
                Line::from(Span::styled(
                    format!("YOUR NEXT STEP / HSK {level}"),
                    Style::default().fg(PURPLE),
                )),
                Line::from(Span::styled(
                    if done {
                        "Daily goal complete!"
                    } else {
                        "Make one word yours."
                    },
                    Style::default().fg(WHITE).bold(),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "3 challenges · +20 XP",
                    Style::default().fg(GOLD),
                )),
            ]
        } else {
            vec![
                Line::from(Span::styled(
                    format!("YOUR NEXT STEP  /  HSK {level}"),
                    Style::default().fg(PURPLE),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    if done {
                        "You showed up. That counts."
                    } else {
                        "Make one word yours."
                    },
                    Style::default().fg(WHITE).bold(),
                )),
                Line::from("Learn it. Try it. Remember it."),
                Line::from(""),
                Line::from(Span::styled(
                    "3 quick challenges    +20 XP",
                    Style::default().fg(GOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    if done {
                        "Daily goal complete. Keep growing?"
                    } else {
                        "One small lesson is all it takes."
                    },
                    Style::default().fg(MUTED),
                )),
            ]
        };
        let mascot = include_str!("../assets/owl.txt");
        let mascot_width = mascot.lines().map(str::len).max().unwrap_or(0) as u16;
        let mascot_height = mascot.lines().count() as u16;
        let text_width = lines.iter().map(Line::width).max().unwrap_or(0) as u16;
        let content_width = (mascot_width + 4 + text_width).min(inner.width.saturating_sub(2));
        let content = Rect::new(
            inner.x + (inner.width - content_width) / 2,
            inner.y + (inner.height - content_height) / 2,
            content_width,
            content_height,
        );
        let columns = Layout::horizontal([
            Constraint::Length(mascot_width),
            Constraint::Length(4),
            Constraint::Min(0),
        ])
        .split(content);
        let owl_area = Rect::new(
            columns[0].x,
            columns[0].y + columns[0].height.saturating_sub(mascot_height) / 2,
            mascot_width.min(columns[0].width),
            mascot_height.min(columns[0].height),
        );
        // Center the whole group while preserving the ASCII art's internal spacing.
        frame.render_widget(
            Paragraph::new(mascot).style(Style::default().fg(GREEN)),
            owl_area,
        );
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), columns[2]);
        frame.render_widget(
            Gauge::default()
                .gauge_style(Style::default().fg(GREEN).bg(PANEL))
                .ratio((self.progress.today_xp(today).min(20) as f64) / 20.0)
                .label(format!(
                    "DAILY GOAL  {} / 20 XP",
                    self.progress.today_xp(today).min(20)
                )),
            parts[1],
        );
        let detail = format!(
            "HSK {level}  ·  Learn → Meaning → Characters → Pinyin\n{} words ready for review",
            self.progress.due_count(today)
        );
        text(frame, parts[2], &detail, MUTED, Alignment::Center);
        button(
            frame,
            parts[3],
            if next.is_none() {
                "COURSE COMPLETE — REVIEW WITH r"
            } else if done {
                "WANT MORE CHALLENGE?  [Enter]"
            } else {
                "START MY DAILY LESSON  [Enter]"
            },
            GREEN,
        );
    }
    fn study(&self, frame: &mut Frame, area: Rect) {
        let lesson = self.lesson.as_ref().unwrap();
        let word = &self.words[lesson.word];
        let parts = Layout::vertical([Constraint::Min(9), Constraint::Length(3)]).split(area);
        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("MEET YOUR WORD  ·  HSK {}", word.level),
                Style::default().fg(PURPLE),
            )),
            Line::from(""),
            Line::from(Span::styled(
                word.characters(self.traditional),
                Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(&word.pinyin, Style::default().fg(GOLD))),
            Line::from(""),
            Line::from(word.meaning.as_str()),
            Line::from(""),
        ];
        if !word.example.is_empty() {
            lines.extend([
                Line::from(Span::styled("IN A SENTENCE", Style::default().fg(MUTED))),
                Line::from(word.sentence(self.traditional)),
                Line::from(word.example_pinyin.as_str()),
                Line::from(word.translation.as_str()),
                Line::from(""),
            ]);
        }
        lines.push(Line::from(Span::styled(
            "Say it aloud. Notice the tone marks.",
            Style::default().fg(MUTED),
        )));
        frame.render_widget(
            Paragraph::new(lines)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true })
                .scroll((self.scroll, 0))
                .block(panel(" LEARN ", PURPLE)),
            parts[0],
        );
        button(frame, parts[1], "GOT IT. LET'S PRACTICE!  [Enter]", GREEN);
    }
    fn question(&self, frame: &mut Frame, area: Rect) {
        let lesson = self.lesson.as_ref().unwrap();
        let word = &self.words[lesson.word];
        let parts = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(5),
            Constraint::Min(6),
            Constraint::Length(4),
        ])
        .split(area);
        let hearts = format!(
            "{}{}",
            "♥ ".repeat(lesson.hearts as usize),
            "♡ ".repeat(3 - lesson.hearts as usize)
        );
        frame.render_widget(
            Gauge::default()
                .gauge_style(Style::default().fg(GREEN).bg(PANEL))
                .ratio(lesson.round as f64 / ROUNDS as f64)
                .label(format!(
                    "{}  ·  {} / {}  ·  {hearts}",
                    if self.review { "REVIEW" } else { "CHALLENGE" },
                    lesson.round + 1,
                    ROUNDS
                )),
            parts[0],
        );
        let (instruction, prompt) = match lesson.round {
            0 => ("Choose the meaning", word.characters(self.traditional)),
            1 => (
                "Choose the Chinese word",
                word.meaning.split(';').next().unwrap_or(&word.meaning),
            ),
            _ => (
                "Choose the pronunciation",
                word.characters(self.traditional),
            ),
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(instruction, Style::default().fg(MUTED))),
                Line::from(Span::styled(prompt, Style::default().fg(WHITE).bold())),
            ])
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
            parts[1],
        );
        let grid_rows = Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(parts[2]);
        for (position, &index) in lesson.choices.iter().enumerate() {
            let cells =
                Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(grid_rows[position / 2]);
            let selected = lesson.selected == position;
            let color = if matches!(self.screen, Screen::Feedback(_)) && index == lesson.word {
                GREEN
            } else if selected && self.screen == Screen::Feedback(false) {
                RED
            } else if selected {
                PURPLE
            } else {
                MUTED
            };
            let label = lesson.label(&self.words[index], self.traditional);
            frame.render_widget(
                Paragraph::new(label)
                    .wrap(Wrap { trim: true })
                    .style(Style::default().fg(if selected { WHITE } else { MUTED }))
                    .block(panel(
                        &format!(" {} {} ", position + 1, if selected { "●" } else { "○" }),
                        color,
                    )),
                cells[position % 2],
            );
        }
        match self.screen {
            Screen::Feedback(true) => button(
                frame,
                parts[3],
                "✓ NICE WORK!  Press Enter to continue",
                GREEN,
            ),
            Screen::Feedback(false) => {
                let answer = lesson.label(word, self.traditional);
                frame.render_widget(Paragraph::new(format!("Not quite. Correct answer: {answer}\nPress Enter to try again. You can do this."))
                    .wrap(Wrap { trim: true }).style(Style::default().fg(RED)).block(panel(" KEEP GOING ", RED)), parts[3]);
            }
            _ => button(frame, parts[3], "CHECK MY ANSWER  [Enter]", GREEN),
        }
    }
    fn complete(&self, frame: &mut Frame, area: Rect) {
        let lesson = self.lesson.as_ref().unwrap();
        let word = &self.words[lesson.word];
        let headline = if lesson.mistakes == 0 {
            "PERFECT LESSON!"
        } else {
            "YOU STUCK WITH IT!"
        };
        let reward = if self.earned == 0 {
            "Already earned XP for this word today. Practice still counts!".to_string()
        } else {
            format!("+{} XP  ·  {} challenges complete", self.earned, ROUNDS)
        };
        let body = format!(
            "\n{headline}\n\n{}  ·  {}\n{}\n\n{reward}\n\nWant more challenge?\nLearn another word, or press r to strengthen your memory.",
            word.characters(self.traditional),
            word.pinyin,
            word.meaning
        );
        let parts = Layout::vertical([Constraint::Min(10), Constraint::Length(3)]).split(area);
        frame.render_widget(
            Paragraph::new(body)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true })
                .style(Style::default().fg(GREEN))
                .block(panel(" ✦ LESSON COMPLETE ✦ ", GREEN)),
            parts[0],
        );
        button(frame, parts[1], "LET'S DO ONE MORE  [Enter]", GREEN);
    }
    fn message(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: &str,
        body: &str,
        action: &str,
        color: Color,
    ) {
        let parts = Layout::vertical([Constraint::Min(8), Constraint::Length(3)]).split(area);
        frame.render_widget(
            Paragraph::new(format!("\n{title}\n\n{body}"))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true })
                .block(panel(" YOUR PATH ", color)),
            parts[0],
        );
        button(frame, parts[1], action, color);
    }
}
fn panel(title: &str, color: Color) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color))
        .style(Style::default().bg(PANEL).fg(WHITE))
        .title(title.to_owned())
}
fn text(frame: &mut Frame, area: Rect, value: &str, color: Color, alignment: Alignment) {
    frame.render_widget(
        Paragraph::new(value)
            .style(Style::default().fg(color))
            .alignment(alignment)
            .wrap(Wrap { trim: true }),
        area,
    );
}
fn button(frame: &mut Frame, area: Rect, label: &str, color: Color) {
    frame.render_widget(
        Paragraph::new(label)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(BG).bg(color).bold())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(color).bg(color))
                    .style(Style::default().bg(color)),
            ),
        area,
    );
}
struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
    }
}
pub fn run(
    words: &[Word],
    progress: &mut Progress,
    path: &Path,
    traditional: bool,
    review: bool,
    audio: Audio,
    open_analytics: bool,
) -> io::Result<()> {
    enable_raw_mode()?;
    let _guard = TerminalGuard;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut app = App {
        words,
        progress,
        path,
        traditional,
        screen: Screen::Home,
        lesson: None,
        review: false,
        earned: 0,
        scroll: 0,
        audio,
        analytics: analytics::State::default(),
    };
    if open_analytics {
        app.screen = Screen::Analytics;
    }
    if review {
        app.start(false, true)?;
    }
    loop {
        app.audio.poll();
        terminal.draw(|frame| app.render(frame))?;
        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        if key.code == KeyCode::Char('q')
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            break;
        }
        // Do not take lesson actions while a resize notice hides the controls.
        let size = terminal.size()?;
        if size.width >= 60 && size.height >= 26 {
            app.handle(key.code)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);

    fn with_app(test: impl FnOnce(&mut App<'_>)) {
        let words = crate::course::bundled();
        let mut progress = Progress::new(Local::now().date_naive());
        let directory = std::env::temp_dir().join(format!(
            "xiuliu-ui-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("progress.json");
        let mut app = App {
            words: &words,
            progress: &mut progress,
            path: &path,
            traditional: false,
            screen: Screen::Home,
            lesson: None,
            review: false,
            earned: 0,
            scroll: 0,
            audio: Audio::silent(),
            analytics: analytics::State::default(),
        };
        test(&mut app);
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn full_lesson_saves_once_and_offers_an_extra_word() {
        with_app(|app| {
            app.handle(KeyCode::Enter).unwrap();
            assert!(app.screen == Screen::Study);
            app.handle(KeyCode::Enter).unwrap();
            for _ in 0..ROUNDS {
                assert!(app.screen == Screen::Question);
                let lesson = app.lesson.as_mut().unwrap();
                lesson.selected = lesson
                    .choices
                    .iter()
                    .position(|&i| i == lesson.word)
                    .unwrap();
                app.handle(KeyCode::Enter).unwrap();
                assert!(app.screen == Screen::Feedback(true));
                assert_eq!(app.progress.xp(), 0);
                app.handle(KeyCode::Enter).unwrap();
            }
            assert!(app.screen == Screen::Complete);
            assert_eq!(app.progress.xp(), 20);
            let saved = Progress::load(app.path, Local::now().date_naive(), app.words).unwrap();
            assert_eq!(saved.xp(), 20);
            app.handle(KeyCode::Enter).unwrap();
            assert!(app.screen == Screen::Study);
            assert_eq!(app.lesson.as_ref().unwrap().word, 1);
            assert_eq!(app.progress.xp(), 20);
        });
    }
    #[test]
    fn three_mistakes_allow_a_free_retry_without_rewards() {
        with_app(|app| {
            app.start(false, false).unwrap();
            app.handle(KeyCode::Enter).unwrap();
            for _ in 0..3 {
                let lesson = app.lesson.as_mut().unwrap();
                lesson.selected = lesson
                    .choices
                    .iter()
                    .position(|&i| i != lesson.word)
                    .unwrap();
                app.handle(KeyCode::Enter).unwrap();
                assert!(app.screen == Screen::Feedback(false));
                app.handle(KeyCode::Enter).unwrap();
            }
            assert!(app.screen == Screen::Retry);
            assert_eq!(app.progress.xp(), 0);
            app.handle(KeyCode::Enter).unwrap();
            assert!(app.screen == Screen::Study);
            assert_eq!(app.lesson.as_ref().unwrap().hearts, 3);
        });
    }
    #[test]
    fn analytics_navigation_renders_all_levels_and_preserves_history() {
        with_app(|app| {
            let today = Local::now().date_naive();
            app.progress.complete(&app.words[0].simplified, today);
            let before = serde_json::to_vec(app.progress).unwrap();
            app.handle(KeyCode::Char('g')).unwrap();
            assert!(app.screen == Screen::Analytics);
            for (width, height) in [(60, 26), (80, 30), (120, 36)] {
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                for key in ['1', '2', '3'] {
                    app.handle(KeyCode::Char(key)).unwrap();
                    for marker in ["ACTIVITY / XP", "HSK 7–9", "Upcoming:"] {
                        terminal.draw(|f| app.render(f)).unwrap();
                        let rendered = terminal
                            .backend()
                            .buffer()
                            .content
                            .iter()
                            .map(|cell| cell.symbol())
                            .collect::<String>();
                        assert!(
                            rendered.contains(marker),
                            "missing {marker} at {width}x{height}"
                        );
                        assert!(rendered.contains("Esc home"));
                        app.handle(KeyCode::Tab).unwrap();
                    }
                }
            }
            app.handle(KeyCode::Left).unwrap();
            assert!(app.analytics.tab == analytics::Tab::Review);
            app.handle(KeyCode::Right).unwrap();
            assert!(app.analytics.tab == analytics::Tab::Overview);
            app.handle(KeyCode::Esc).unwrap();
            assert!(app.screen == Screen::Home);
            assert_eq!(serde_json::to_vec(app.progress).unwrap(), before);
            assert!(!app.path.exists());
        });
    }
    #[test]
    fn render_screens_at_supported_sizes() {
        with_app(|app| {
            app.lesson = Some(Lesson::new(0, app.words, 42, false));
            for (width, height) in [(60, 26), (80, 30), (100, 36)] {
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                for (screen, marker) in [
                    (Screen::Home, "START MY DAILY"),
                    (Screen::Study, "GOT IT."),
                    (Screen::Question, "CHECK MY ANSWER"),
                    (Screen::Feedback(false), "KEEP GOING"),
                    (Screen::Complete, "ONE MORE"),
                    (Screen::Retry, "TRY AGAIN"),
                    (Screen::Analytics, "ACTIVITY / XP"),
                ] {
                    app.screen = screen;
                    terminal.draw(|f| app.render(f)).unwrap();
                    let buffer = terminal.backend().buffer();
                    let rendered = buffer
                        .content
                        .iter()
                        .map(|cell| cell.symbol())
                        .collect::<String>();
                    assert!(
                        rendered.contains(marker),
                        "missing {marker} at {width}x{height}"
                    );
                    if width == 100
                        && matches!(
                            screen,
                            Screen::Home | Screen::Question | Screen::Complete | Screen::Analytics
                        )
                        && let Ok(directory) = std::env::var("XIULIU_PREVIEW_DIR")
                    {
                        std::fs::create_dir_all(&directory).unwrap();
                        let name = match screen {
                            Screen::Home => "home",
                            Screen::Question => "challenge",
                            Screen::Analytics => "analytics",
                            _ => "complete",
                        };
                        let mut svg = format!(
                            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\"><rect width=\"100%\" height=\"100%\" fill=\"#111723\"/>",
                            width * 10,
                            height * 20
                        );
                        let color = |c: Color| match c {
                            Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
                            _ => "#ecf2f9".into(),
                        };
                        let mut foreground = String::new();
                        for y in 0..height {
                            for x in 0..width {
                                let cell = &buffer[(x, y)];
                                let glyph = cell
                                    .symbol()
                                    .replace('&', "&amp;")
                                    .replace('<', "&lt;")
                                    .replace('>', "&gt;");
                                svg.push_str(&format!("<rect x=\"{}\" y=\"{}\" width=\"10\" height=\"20\" fill=\"{}\"/>", x*10,y*20,color(cell.bg)));
                                if !glyph.trim().is_empty() {
                                    foreground.push_str(&format!("<text x=\"{}\" y=\"{}\" font-family=\"Menlo, monospace\" font-size=\"15\" fill=\"{}\">{glyph}</text>",x*10,y*20+16,color(cell.fg)));
                                }
                            }
                        }
                        svg.push_str(&foreground);
                        svg.push_str("</svg>");
                        std::fs::write(format!("{directory}/{name}.svg"), svg).unwrap();
                    }
                }
            }
        });
    }
}
