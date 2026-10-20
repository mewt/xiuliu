mod analytics;
mod audio;
mod course;
mod lesson;
mod mimo;
mod progress;
mod storage;
mod ui;

use chrono::Local;
use course::Word;
use progress::Progress;
use std::{
    env, fs,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
};

const HELP: &str = "xiuliu — one Mandarin word a day

Usage: xiuliu [today|learn|review|stats|speak|analytics] [--traditional] [--data-dir PATH]

  (no command)   Open the interactive app; print today's word when piped
  today          Print today's word, pinyin, and example
  learn          Mark today's word learned
  review         Practice learned words with challenges (requires a terminal)
  analytics      Activity heatmap and learning dashboard (text summary when piped)
  stats          Print your learned count and daily streak
  speak          Pronounce today's word (macOS; --slow / --sentence optional)
  --mute         Start the interactive app silently
  --tts PROVIDER Speech provider: local (default) or mimo (uses .env)
  --voice VOICE  MiMo voice: female, male, bingtang, moli, suda, baihua
  --traditional  Display traditional Chinese characters
  --data-dir     Store progress in a custom directory
  -h, --help     Show this help

App keys: Enter continue/check • 1–4 choose • r review • g analytics • Esc home • q quit
";

#[derive(Default)]
struct Options {
    command: Option<String>,
    traditional: bool,
    data_dir: Option<PathBuf>,
    help: bool,
    muted: bool,
    slow: bool,
    sentence: bool,
    mimo: bool,
    voice: Option<String>,
}

fn options() -> io::Result<Options> {
    let mut options = Options::default();
    let mut args = env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("-h" | "--help") => options.help = true,
            Some("--mute") => options.muted = true,
            Some("--slow") => options.slow = true,
            Some("--sentence") => options.sentence = true,
            Some("--traditional") => options.traditional = true,
            Some("--voice") => {
                let value = args
                    .next()
                    .ok_or_else(|| io::Error::other("--voice requires a voice name"))?;
                let value = value
                    .to_str()
                    .ok_or_else(|| io::Error::other("Invalid voice name"))?;
                options.voice = Some(mimo::voice_id(value)?.into());
            }
            Some("--tts") => {
                options.mimo = match args.next().as_deref().and_then(|value| value.to_str()) {
                    Some("local") => false,
                    Some("mimo") => true,
                    _ => return Err(io::Error::other("--tts requires local or mimo")),
                };
            }
            Some("--data-dir") => {
                let path = args
                    .next()
                    .filter(|p| !p.is_empty())
                    .ok_or_else(|| io::Error::other("--data-dir requires a path"))?;
                options.data_dir = Some(path.into());
            }
            Some(command @ ("today" | "learn" | "review" | "stats" | "speak" | "analytics"))
                if options.command.is_none() =>
            {
                options.command = Some(command.to_owned());
            }
            _ => {
                return Err(io::Error::other(format!(
                    "Unknown or extra argument: {}. Use --help.",
                    arg.to_string_lossy()
                )));
            }
        }
    }
    if options.voice.is_some() && !options.mimo {
        return Err(io::Error::other(
            "--voice requires --tts mimo; local speech uses your system voice.",
        ));
    }
    if (options.slow || options.sentence) && options.command.as_deref() != Some("speak") {
        return Err(io::Error::other(
            "--slow and --sentence are only available with speak",
        ));
    }
    Ok(options)
}

fn default_data_dir() -> io::Result<PathBuf> {
    // Keep the original data directory so rebranding never resets a profile.
    for variable in ["XDG_DATA_HOME", "LOCALAPPDATA"] {
        if let Some(path) = env::var_os(variable)
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
        {
            return Ok(path.join("1day1word"));
        }
    }
    env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".local/share/1day1word"))
        .ok_or_else(|| io::Error::other("Cannot locate a home directory. Use --data-dir PATH."))
}

fn speech(options: &Options, directory: &Path) -> io::Result<audio::Audio> {
    let audio = audio::Audio::new(options.traditional, options.muted);
    if options.mimo && (!options.muted || options.command.as_deref() != Some("speak")) {
        let config = mimo::Config::load(Path::new(".env"), options.voice.as_deref())?
            .with_cache_dir(directory.join("audio"));
        audio.with_mimo(config)
    } else {
        Ok(audio)
    }
}

// Hold a lock throughout a session so two apps cannot overwrite each other's progress.
struct SessionLock(PathBuf);

impl SessionLock {
    fn acquire(directory: &Path) -> io::Result<Self> {
        fs::create_dir_all(directory)?;
        let path = directory.join("session.lock");
        let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&path)
            .map_err(|e| if e.kind() == io::ErrorKind::AlreadyExists {
                io::Error::other(format!("Another session may be running. Close it first. If a previous session crashed, remove {} only after confirming no session is running.", path.display()))
            } else { e })?;
        let lock = Self(path);
        writeln!(file, "{}", std::process::id())?;
        Ok(lock)
    }
}

impl Drop for SessionLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn print_word(word: &Word, index: usize, traditional: bool, done: bool) {
    println!(
        "\n  一天一词 · HSK 1\n  {} · Lesson {}/{} · Word {}/{}{}\n",
        Local::now().date_naive(),
        course::chapter(index),
        course::chapter_count(),
        course::chapter_progress(index).0,
        course::chapter_progress(index).1,
        if done { " · Learned ✓" } else { "" }
    );
    println!(
        "  {}  {}\n  {}\n",
        word.characters(traditional),
        word.pinyin,
        word.meaning
    );
    if !word.example.is_empty() {
        println!(
            "  {}\n  {}\n  {}\n",
            word.sentence(traditional),
            word.example_pinyin,
            word.translation
        );
    }
    if !done {
        println!("  Run `xiuliu learn` when you are ready.\n");
    }
}

fn show_today(words: &[Word], index: Option<usize>, traditional: bool, progress: &Progress) {
    if let Some(index) = index {
        print_word(
            &words[index],
            index,
            traditional,
            progress.learned.contains_key(&words[index].simplified),
        );
    } else {
        println!("Course complete! Practice your learned words with `xiuliu review`.");
    }
}

fn run() -> io::Result<()> {
    let options = options()?;
    if options.help {
        print!("{HELP}");
        return Ok(());
    }
    let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();
    if options.command.as_deref() == Some("review") && !interactive {
        return Err(io::Error::other(
            "Review requires an interactive terminal. Use `today` to print a lesson.",
        ));
    }
    let directory = match options.data_dir.clone() {
        Some(path) => path,
        None => default_data_dir()?,
    };
    let _lock = SessionLock::acquire(&directory)?;
    let path = directory.join("progress.json");
    let words = course::load(&directory)?;
    let today = Local::now().date_naive();
    let mut progress = Progress::load(&path, today, &words)?;
    if options.command.as_deref() == Some("analytics") {
        if interactive {
            return ui::run(
                &words,
                &mut progress,
                &path,
                options.traditional,
                false,
                speech(&options, &directory)?,
                true,
            );
        }
        println!("{}", analytics::plain(&progress, &words, today));
        return Ok(());
    }
    let index = progress.today_word(today, &words);
    progress.save(&path)?;
    match options.command.as_deref() {
        Some("today") => show_today(&words, index, options.traditional, &progress),
        Some("learn") => {
            if let Some(index) = index {
                let xp = progress.complete(&words[index].simplified, today);
                progress.save(&path)?;
                println!(
                    "Learned {} ({}) · +{} XP · {}-day streak. Want more challenge? Run xiuliu!",
                    words[index].characters(options.traditional),
                    words[index].pinyin,
                    xp,
                    progress.streak(today)
                );
            } else {
                println!("You have learned the whole course! Open review to keep practicing.");
            }
        }
        Some("speak") => {
            let index = index.ok_or_else(|| {
                io::Error::other("Course complete. Use interactive review to hear learned words.")
            })?;
            let word = &words[index];
            let text = if options.sentence {
                word.sentence(options.traditional)
            } else {
                word.characters(options.traditional)
            };
            if text.is_empty() {
                return Err(io::Error::other("This word has no example sentence yet."));
            }
            let mut audio = speech(&options, &directory)?;
            let pinyin = if options.sentence {
                &word.example_pinyin
            } else {
                &word.pinyin
            };
            audio.speak(text, pinyin, options.slow);
            audio.wait()?;
        }
        Some("stats") => println!(
            "{} / {} words learned\n{} XP\n{}-day streak\n{} study days\n{} words due for review\nStarted: {}\nProgress: {}",
            progress.active_learned_count(&words),
            words.len(),
            progress.xp(),
            progress.streak(today),
            progress.days.len(),
            progress.due_count(today),
            progress.started_on,
            path.display()
        ),
        None if !interactive => show_today(&words, index, options.traditional, &progress),
        _ => ui::run(
            &words,
            &mut progress,
            &path,
            options.traditional,
            options.command.as_deref() == Some("review"),
            speech(&options, &directory)?,
            false,
        )?,
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("xiuliu: {error}");
        std::process::exit(1);
    }
}
