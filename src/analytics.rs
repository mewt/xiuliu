use crate::{
    course::{self, Word},
    progress::Progress,
    ui::{BG, GOLD, GREEN, MUTED, PANEL, PURPLE, WHITE},
};
use chrono::{Datelike, Days, NaiveDate};
use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Gauge, Paragraph, Wrap},
};

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub enum Period {
    #[default]
    All,
    Week,
    Month,
}
impl Period {
    fn start(self, progress: &Progress, today: NaiveDate) -> NaiveDate {
        let days = match self {
            Self::All => return progress.started_on,
            Self::Week => 6,
            Self::Month => 29,
        };
        today
            .checked_sub_days(Days::new(days))
            .unwrap_or(NaiveDate::MIN)
            .max(progress.started_on)
    }
    fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Week => "7 Days",
            Self::Month => "30 Days",
        }
    }
}
#[derive(Clone, Copy, Default, PartialEq)]
pub enum Tab {
    #[default]
    Overview,
    Vocabulary,
    Review,
}
impl Tab {
    fn next(self, reverse: bool) -> Self {
        match (self, reverse) {
            (Self::Overview, false) | (Self::Review, true) => Self::Vocabulary,
            (Self::Vocabulary, false) | (Self::Overview, true) => Self::Review,
            _ => Self::Overview,
        }
    }
}
#[derive(Default)]
pub struct State {
    pub tab: Tab,
    pub period: Period,
}
impl State {
    pub fn handle(&mut self, key: KeyCode) {
        match key {
            KeyCode::Tab | KeyCode::Right => self.tab = self.tab.next(false),
            KeyCode::BackTab | KeyCode::Left => self.tab = self.tab.next(true),
            KeyCode::Char('1') => self.period = Period::All,
            KeyCode::Char('2') => self.period = Period::Week,
            KeyCode::Char('3') => self.period = Period::Month,
            _ => {}
        }
    }
}

#[derive(Debug)]
pub struct Summary {
    pub start: NaiveDate,
    pub xp: u64,
    pub new_words: usize,
    pub practices: usize,
    pub reviews: usize,
    pub active_days: usize,
    pub goal_days: usize,
    pub best_streak: usize,
    pub calendar_days: usize,
    pub best_day: Option<(NaiveDate, u64)>,
}
impl Summary {
    pub fn calculate(progress: &Progress, period: Period, today: NaiveDate) -> Self {
        let start = period.start(progress, today);
        let mut summary = Self {
            start,
            xp: 0,
            new_words: 0,
            practices: 0,
            reviews: 0,
            active_days: 0,
            goal_days: 0,
            best_streak: 0,
            calendar_days: today.signed_duration_since(start).num_days().max(0) as usize + 1,
            best_day: None,
        };
        let mut previous: Option<NaiveDate> = None;
        let mut streak = 0;
        for (&date, day) in progress.days.range(start..=today) {
            summary.xp += day.xp;
            summary.practices += day.words.len();
            summary.reviews += day
                .words
                .iter()
                .filter(|id| {
                    progress
                        .learned
                        .get(*id)
                        .is_some_and(|m| m.first_learned < date)
                })
                .count();
            if day.words.is_empty() {
                continue;
            }
            summary.active_days += 1;
            summary.goal_days += usize::from(day.xp >= 20);
            streak = if previous.is_some_and(|d| date.signed_duration_since(d).num_days() == 1) {
                streak + 1
            } else {
                1
            };
            summary.best_streak = summary.best_streak.max(streak);
            previous = Some(date);
            if summary.best_day.is_none_or(|(_, xp)| day.xp > xp) {
                summary.best_day = Some((date, day.xp));
            }
        }
        summary.new_words = progress
            .learned
            .values()
            .filter(|m| m.first_learned >= start && m.first_learned <= today)
            .count();
        summary
    }
}

// Monday-first columns, ending with the current week. Narrow screens show fewer
// weeks and label the exact visible date span instead of silently squashing days.
fn heatmap_start(today: NaiveDate, requested: NaiveDate, capacity: usize) -> (NaiveDate, usize) {
    let current_monday = today
        .checked_sub_days(Days::new(today.weekday().num_days_from_monday() as u64))
        .unwrap_or(today);
    let requested_monday = requested
        .checked_sub_days(Days::new(requested.weekday().num_days_from_monday() as u64))
        .unwrap_or(requested);
    let needed = (current_monday
        .signed_duration_since(requested_monday)
        .num_days()
        .max(0) as usize
        / 7)
        + 1;
    let weeks = needed.min(capacity.max(1));
    let start = current_monday
        .checked_sub_days(Days::new((weeks.saturating_sub(1) * 7) as u64))
        .unwrap_or(current_monday);
    (start, weeks)
}
fn shade(xp: u64) -> Color {
    match xp {
        0 => Color::Rgb(30, 40, 54),
        1..=19 => Color::Rgb(46, 84, 60),
        20..=39 => Color::Rgb(70, 128, 72),
        40..=79 => Color::Rgb(104, 180, 82),
        _ => GREEN,
    }
}
fn paragraph(frame: &mut Frame, area: Rect, value: impl Into<String>, color: Color) {
    frame.render_widget(
        Paragraph::new(value.into())
            .style(Style::default().fg(color))
            .wrap(Wrap { trim: true }),
        area,
    );
}
fn box_block(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(title.to_owned())
        .border_style(Style::default().fg(MUTED))
        .style(Style::default().bg(PANEL).fg(WHITE))
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    words: &[Word],
    progress: &Progress,
    state: &State,
    today: NaiveDate,
) {
    let width = area.width.min(120);
    let root = Rect::new(
        area.x + (area.width - width) / 2 + 2,
        area.y + 1,
        width.saturating_sub(4),
        area.height.saturating_sub(2),
    );
    let rows = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Min(17),
        Constraint::Length(2),
    ])
    .split(root);
    paragraph(
        frame,
        rows[0],
        "✦ XIULIU  /  YOUR LEARNING IN NUMBERS",
        GREEN,
    );
    let tabs = Layout::horizontal([Constraint::Min(28), Constraint::Length(25)]).split(rows[1]);
    let tab_items = [
        (Tab::Overview, "Overview"),
        (Tab::Vocabulary, "Words"),
        (Tab::Review, "Review"),
    ];
    let spans: Vec<_> = tab_items
        .into_iter()
        .flat_map(|(tab, name)| {
            [
                Span::styled(
                    format!(" {name} "),
                    if tab == state.tab {
                        Style::default().bg(PURPLE).fg(BG).bold()
                    } else {
                        Style::default().fg(MUTED)
                    },
                ),
                Span::raw(" "),
            ]
        })
        .collect();
    frame.render_widget(Paragraph::new(Line::from(spans)), tabs[0]);
    let spans: Vec<_> = [Period::All, Period::Week, Period::Month]
        .into_iter()
        .flat_map(|period| {
            [
                Span::styled(
                    format!(" {} ", period.label()),
                    if period == state.period {
                        Style::default().bg(GREEN).fg(BG).bold()
                    } else {
                        Style::default().fg(MUTED)
                    },
                ),
                Span::raw(" "),
            ]
        })
        .collect();
    frame.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Right),
        tabs[1],
    );
    let summary = Summary::calculate(progress, state.period, today);
    match state.tab {
        Tab::Overview => overview(frame, rows[2], progress, state, &summary, today),
        Tab::Vocabulary => vocabulary(frame, rows[2], words, progress, &summary, today),
        Tab::Review => review(frame, rows[2], progress, &summary, today),
    }
    paragraph(
        frame,
        rows[3],
        "Tab / ← → tabs   1 All · 2 Week · 3 Month\nEsc home · q quit",
        MUTED,
    );
}

fn overview(
    frame: &mut Frame,
    area: Rect,
    progress: &Progress,
    state: &State,
    summary: &Summary,
    today: NaiveDate,
) {
    let parts = Layout::vertical([Constraint::Length(12), Constraint::Min(5)]).split(area);
    heatmap(frame, parts[0], progress, state.period, today);
    let columns = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(parts[1]);
    let average = summary.xp as f64 / summary.calendar_days as f64;
    paragraph(
        frame,
        columns[0],
        format!(
            "XP earned:        {}\nNew words:        {}\nWord practices:   {}\nReview practices: {}\nDaily average:    {average:.1} XP",
            summary.xp, summary.new_words, summary.practices, summary.reviews
        ),
        WHITE,
    );
    paragraph(
        frame,
        columns[1],
        format!(
            "Active days:     {} / {}\nGoal days:       {}\nCurrent streak:  {} days\nBest in period:  {} days\n{}",
            summary.active_days,
            summary.calendar_days,
            summary.goal_days,
            progress.streak(today),
            summary.best_streak,
            if summary.active_days == 0 {
                "Start your first lesson.".to_owned()
            } else {
                format!(
                    "Best day: {}",
                    summary.best_day.map_or("—".into(), |(date, _)| date
                        .format("%d %b %Y")
                        .to_string())
                )
            }
        ),
        GOLD,
    );
}

fn heatmap(frame: &mut Frame, area: Rect, progress: &Progress, period: Period, today: NaiveDate) {
    let stride = if area.width >= 111 { 2 } else { 1 };
    let capacity = ((area.width.saturating_sub(5)) / stride).max(1) as usize;
    let requested = match period {
        Period::All => today.checked_sub_days(Days::new(364)).unwrap_or(today),
        _ => period.start(progress, today),
    };
    let (start, weeks) = heatmap_start(today, requested, capacity);
    let visible = start.max(requested);
    paragraph(
        frame,
        Rect::new(area.x, area.y, area.width, 1),
        format!(
            "ACTIVITY / XP   {} — {}",
            visible.format("%d %b %Y"),
            today.format("%d %b %Y")
        ),
        MUTED,
    );
    let x = area.x + 4;
    let y = area.y + 2;
    let mut last_month = None;
    let mut next_label_x = x;
    for week in 0..weeks {
        let monday = start
            .checked_add_days(Days::new(week as u64 * 7))
            .unwrap_or(today);
        let month = (monday.year(), monday.month());
        let column_x = x + week as u16 * stride;
        if last_month != Some(month) && column_x >= next_label_x && column_x + 3 <= area.right() {
            paragraph(
                frame,
                Rect::new(column_x, area.y + 1, 3, 1),
                monday.format("%b").to_string(),
                MUTED,
            );
            last_month = Some(month);
            next_label_x = column_x + 4;
        }
        for weekday in 0..7 {
            let date = monday.checked_add_days(Days::new(weekday)).unwrap_or(today);
            if date < visible || date > today {
                continue;
            }
            let xp = progress.days.get(&date).map_or(0, |day| day.xp);
            frame.render_widget(
                Paragraph::new(if xp == 0 { "·" } else { " " }).style(
                    Style::default()
                        .fg(shade(xp))
                        .bg(if xp == 0 { BG } else { shade(xp) }),
                ),
                Rect::new(column_x, y + weekday as u16, 1, 1),
            );
        }
    }
    for (offset, label) in [(0, "Mon"), (2, "Wed"), (4, "Fri"), (6, "Sun")] {
        paragraph(frame, Rect::new(area.x, y + offset, 3, 1), label, MUTED);
    }
    let legend = Line::from(vec![
        Span::styled("Less ", Style::default().fg(MUTED)),
        Span::styled("· ", Style::default().fg(shade(0))),
        Span::styled(" ", Style::default().bg(shade(5))),
        Span::raw(" "),
        Span::styled(" ", Style::default().bg(shade(20))),
        Span::raw(" "),
        Span::styled(" ", Style::default().bg(shade(40))),
        Span::raw(" "),
        Span::styled(" ", Style::default().bg(shade(80))),
        Span::raw(" "),
        Span::styled(
            " More · 0 / <20 / <40 / <80 / 80+ XP",
            Style::default().fg(MUTED),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(legend),
        Rect::new(area.x, y + 7, area.width, 1),
    );
    paragraph(
        frame,
        Rect::new(area.x, y + 8, area.width, 1),
        "Selected period totals; each word counts once/day.",
        MUTED,
    );
}

fn vocabulary(
    frame: &mut Frame,
    area: Rect,
    words: &[Word],
    progress: &Progress,
    summary: &Summary,
    today: NaiveDate,
) {
    let block = box_block(" VOCABULARY / CURRENT COLLECTION ");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    paragraph(
        frame,
        Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(2), 2),
        format!(
            "{} / {} HSK 1 words learned · {} new in selected period\nProgress by textbook lesson",
            progress.active_learned_count(words),
            words.len(),
            summary.new_words
        ),
        WHITE,
    );
    for chapter in 1..=course::chapter_count() {
        let chapter_words = &words[course::chapter_range(chapter)];
        let learned = chapter_words
            .iter()
            .filter(|word| progress.learned.contains_key(&word.simplified))
            .count();
        let new = chapter_words
            .iter()
            .filter(|word| {
                progress
                    .learned
                    .get(&word.simplified)
                    .is_some_and(|memory| {
                        memory.first_learned >= summary.start && memory.first_learned <= today
                    })
            })
            .count();
        let y = inner.y + 3 + (chapter as u16 - 1);
        if y >= inner.bottom() {
            break;
        }
        frame.render_widget(
            Gauge::default()
                .gauge_style(Style::default().fg(GREEN).bg(PANEL))
                .ratio(learned as f64 / chapter_words.len() as f64)
                .label(format!(
                    "Lesson {chapter} · {}   {learned}/{}   +{new}",
                    course::chapter_title(chapter),
                    chapter_words.len()
                )),
            Rect::new(inner.x + 1, y, inner.width.saturating_sub(2), 1),
        );
    }
}

fn review(frame: &mut Frame, area: Rect, progress: &Progress, summary: &Summary, today: NaiveDate) {
    let block = box_block(" REVIEW / MEMORY PRACTICE ");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    paragraph(
        frame,
        Rect::new(inner.x + 1, inner.y, inner.width.saturating_sub(2), 3),
        format!(
            "{} review practices in selected period\n{} words due now · {} words in your collection\nUpcoming: excludes words already due today",
            summary.reviews,
            progress.due_count(today),
            progress.learned.len()
        ),
        WHITE,
    );
    let counts: Vec<_> = (1..=7)
        .map(|offset| {
            let date = today.checked_add_days(Days::new(offset)).unwrap_or(today);
            (
                date,
                progress
                    .learned
                    .values()
                    .filter(|m| m.due_on == date)
                    .count(),
            )
        })
        .collect();
    let maximum = counts
        .iter()
        .map(|(_, count)| *count)
        .max()
        .unwrap_or(0)
        .max(1);
    for (i, (date, count)) in counts.into_iter().enumerate() {
        let y = inner.y + 4 + i as u16;
        if y >= inner.bottom() {
            break;
        }
        frame.render_widget(
            Gauge::default()
                .gauge_style(Style::default().fg(PURPLE).bg(PANEL))
                .ratio(count as f64 / maximum as f64)
                .label(format!("{}   {} words", date.format("%a %d %b"), count)),
            Rect::new(inner.x + 1, y, inner.width.saturating_sub(2), 1),
        );
    }
    if inner.height >= 14 {
        let long = progress
            .learned
            .values()
            .filter(|m| m.interval_days >= 8)
            .count();
        paragraph(
            frame,
            Rect::new(inner.x + 1, inner.y + 12, inner.width.saturating_sub(2), 2),
            format!(
                "{long} words have review intervals of 8+ days.\nFrom home: r to practice. Schedule as of today."
            ),
            MUTED,
        );
    }
}

pub fn plain(progress: &Progress, words: &[Word], today: NaiveDate) -> String {
    let s = Summary::calculate(progress, Period::All, today);
    format!(
        "XIULIU ANALYTICS · All time\n{} — {}\n{} XP\n{} / {} words learned\n{} word practices ({} reviews)\n{} active days · {} goal days\n{}-day current streak · {}-day best streak\n{} words due for review\n\nOpen in a terminal for the activity heatmap and filters.",
        s.start,
        today,
        s.xp,
        progress.learned.len(),
        words.len(),
        s.practices,
        s.reviews,
        s.active_days,
        s.goal_days,
        progress.streak(today),
        s.best_streak,
        progress.due_count(today)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn date(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }
    #[test]
    fn filters_are_inclusive_and_repeats_are_not_new_lessons() {
        let mut p = Progress::new(date("2026-10-01"));
        p.complete("你好", date("2026-10-01"));
        p.complete("你好", date("2026-10-10"));
        p.complete("谢谢", date("2026-10-10"));
        p.complete("谢谢", date("2026-10-10"));
        p.complete("你好", date("2026-10-11"));
        p.complete("我", date("2026-10-16"));
        let s = Summary::calculate(&p, Period::Week, date("2026-10-16"));
        assert_eq!(s.start, date("2026-10-10"));
        assert_eq!((s.xp, s.new_words, s.practices, s.reviews), (50, 2, 4, 2));
        assert_eq!(
            (s.active_days, s.goal_days, s.best_streak, s.calendar_days),
            (3, 2, 2, 7)
        );
        assert_eq!(
            Summary::calculate(&p, Period::All, date("2026-10-16")).xp,
            70
        );
    }
    #[test]
    fn heatmap_keeps_weekdays_aligned_across_leap_day_and_year_end() {
        for end in [date("2024-03-03"), date("2024-03-04"), date("2026-01-01")] {
            let requested = end.checked_sub_days(Days::new(29)).unwrap();
            let (start, weeks) = heatmap_start(end, requested, 53);
            assert_eq!(start.weekday(), chrono::Weekday::Mon);
            assert!(start <= requested);
            assert!(end.signed_duration_since(start).num_days() < weeks as i64 * 7);
        }
        let (start, weeks) = heatmap_start(date("2026-10-16"), date("2025-10-16"), 10);
        assert_eq!(weeks, 10);
        assert_eq!(start, date("2026-08-10"));
    }
    #[test]
    fn empty_history_and_short_new_profiles_have_no_invented_stats() {
        let p = Progress::new(date("2026-10-16"));
        let s = Summary::calculate(&p, Period::Month, date("2026-10-16"));
        assert_eq!(
            (s.xp, s.active_days, s.new_words, s.calendar_days),
            (0, 0, 0, 1)
        );
        assert!(s.best_day.is_none());
    }
}
