# ✦ Xiuliu · 一天一词

A free Mandarin learning app for your terminal, built with Rust and Ratatui.
Xiuliu, your little ASCII owl, guides the daily path. Learn a word, beat three short
challenges, then decide: **want more challenge?**
The default offline mode needs no accounts, API keys, subscriptions, or AI credits.

## Install and start

Install from this checkout:

```sh
cargo install --path . --locked
xiuliu
```

Once the crate is published, install it with `cargo install xiuliu --locked`.
To run directly from source:

```sh
cargo run --release
```

Use a current stable Rust toolchain and a terminal with Chinese font support.
The interface needs at least **60 × 26** cells; **90 × 32** or larger is recommended.

The app currently follows the 15-lesson progression of **HSK Standard Course 1**, with 166 unique lesson vocabulary items. It works offline immediately. Simplified characters are the default; use `cargo run --release -- --traditional` for traditional characters.

## A lesson in a minute

1. **Meet your word:** characters, tone-marked pinyin, and English meaning.
2. **Try three challenges:** choose the meaning, Chinese characters, and pronunciation.
3. **Learn from feedback:** incorrect answers reveal the correct answer; try again.
4. **Collect 20 XP:** completing a new word saves it to your collection.
5. **Want more challenge?** Continue to the next word, or review what you know.

You get three hearts per attempt. Running out offers a free retry, with no waiting
or lost progress. Partial lessons do not earn XP or mark words learned.

| Key | Action |
| --- | --- |
| Enter | Start, check an answer, or continue |
| `1`–`4`, arrows, Tab | Select an answer; Enter submits it |
| ↑ / ↓ on the study card | Scroll longer definitions and examples |
| `?` during a question | Read the full selected answer option; Enter returns |
| `s` / `S` | Replay the word / replay slowly |
| `a` | Hear the example sentence, when available |
| `m` | Mute or unmute speech and chimes for this session |
| `g` on home or completion | Open learning analytics |
| `r` on home or completion | Review learned words, prioritizing due words |
| `n` on completion | Learn another word |
| Esc / `h` | Return home; unfinished challenges restart |
| `q` / Ctrl+C | Quit |

## Hear the words

On **macOS**, a lesson speaks its word when the study card opens. Replay with `s`,
listen slowly with `S`, or press `a` to hear its example sentence. Correct answers,
mistakes, and lesson completion have short feedback chimes. Audio runs in the
background; replay replaces earlier audio, and mute or quitting stops playback.

Speech uses macOS's Mandarin voices (Tingting for simplified Chinese, Meijia for
traditional Chinese when available). Once the system voice is installed, this is
free and offline. Audio is currently macOS-only; other platforms keep the same
text lessons and show an availability message.

```sh
cargo run --release -- speak                    # Hear today's word
cargo run --release -- speak --slow             # Slower pronunciation
cargo run --release -- speak --sentence         # Hear the example, if available
cargo run --release -- --mute                   # Start quietly
```

Use `m` to toggle sound for the current session. If no Mandarin voice is available,
install one through macOS's speech/accessibility settings. `say -v '?'` lists
voices. If playback is silent, check the Mac's output device and volume, and run
from a normal terminal with access to macOS audio services. The app never falls
back to an English or Cantonese voice for Mandarin pronunciation.

### Optional Xiaomi MiMo speech

Keep system speech with `--tts local` (the default), or explicitly enable MiMo:

```sh
cargo run --release -- --tts mimo
cargo run --release -- speak --tts mimo --slow
cargo run --release -- speak --tts mimo --sentence
```

Choose a female or male Mandarin voice for this run:

```sh
cargo run --release -- --tts mimo --voice female
cargo run --release -- --tts mimo --voice male
```

| Voice option | Xiaomi voice | Voice type |
| --- | --- | --- |
| `female` or `bingtang` | 冰糖 | Female (default) |
| `moli` | 茉莉 | Female |
| `male` or `suda` | 苏打 | Male |
| `baihua` | 白桦 | Male |

The same options work with `speak`, `--slow`, and `--sentence`. To save your
preference, set `MIMO_VOICE=male` or `MIMO_VOICE=female` in `.env`; named voices
and Chinese voice IDs also work. `--voice` takes precedence over environment
variables and `.env`, and requires `--tts mimo`. Offline fallback uses your
installed system voice, which may have a different voice type.

For a new setup, copy `.env.example` to `.env` and supply your own `MIMO_API_KEY`.
Existing `.env` files should be retained. Run from that directory: the app reads
only the current directory's `.env`; exported variables take precedence. It does
not execute shell expressions in the file. The supported settings are:

```dotenv
MIMO_API_KEY=your_key_here
MIMO_BASE_URL=https://token-plan-sgp.xiaomimimo.com/v1
MIMO_MODEL=mimo-v2.5-tts
MIMO_VOICE=冰糖
```

MiMo sends the selected word or example sentence to Xiaomi for synthesis. It
requires internet access and an API key valid for the selected endpoint. The
Chinese voice is selected explicitly; normal and slow readings are separate.
Existing `s`, `S`, `a`, and `m` controls work with either provider. This speaks
existing lesson text; it does not generate new sentences or change pinyin.
The lesson's pinyin is sent as pronunciation guidance, separately from the spoken
text, to help disambiguate readings such as `听` (`tīng`, to listen). Sentence
playback uses the example's pinyin when available. Generated pronunciation can
still need checking against the displayed reading.

Requests run in the background with a timeout. Replay uses a small in-memory
cache for the current session. Muting, leaving a lesson, or replacing playback
discards pending audio; an already-sent server request may still finish.
On request failure, the app falls back to the installed Mandarin system voice.
Playback currently requires macOS for both providers. Temporary audio files are
removed after playback or a normal exit. `.env` is ignored by Git and excluded
from the published crate; `.env.example` contains placeholders only.

Xiaomi advertises TTS as free for a limited time; check your plan before use.
See the [MiMo speech documentation](https://mimo.mi.com/docs/en-US/quick-start/usage-guide/audio/speech-synthesis-v2.5)
and [Token Plan endpoints](https://mimo.mi.com/docs/en-US/tokenplan/Token%20Plan/subscription).

## HSK Standard Course 1 lessons

The active course follows the vocabulary order and 15-lesson grouping of the 2014 edition of *HSK Standard Course 1*. It contains simplified/traditional forms, pinyin, learner-friendly meanings, and independently authored example sentences where available. The app does not distribute textbook scans, dialogues, exercises, or example sentences.

Dictionary data is adapted from [Complete HSK Vocabulary](https://github.com/drkameleon/complete-hsk-vocabulary), supplemented with independently written entries needed by the textbook sequence. Lesson IDs and boundaries are explicit rather than computed from a fixed number of words.

The previous full-course `update` command is disabled so an update cannot silently add HSK 2–7 words. Existing progress remains stored by word and review history is preserved. Attribution and data licenses are in [assets/NOTICE.md](assets/NOTICE.md).

## Daily goal, streaks, and review

Your daily word stays fixed for that local calendar day. Missing a day does not
skip vocabulary, and extra lessons are always available. The daily XP goal is 20.
Completing any new lesson or review counts as a study day for your streak.
Yesterday's streak remains active until today ends.

A new word earns **20 XP**. Reviewing an already learned word earns **5 XP** on a
later day. Each word earns XP only once per day, preventing repeated XP farming.
New words are due for review the next day. A successful due review doubles the
interval (2, 4, 8, 16, then 30 days); early practice does not postpone a due date.

## Your learning analytics

Press **g** from home or a completed lesson, or launch directly:

```sh
cargo run --release -- analytics
```

Use **Tab / ← / →** to switch **Overview**, **Words**, and **Review**. Press **1**
for all time, **2** for the last 7 days, or **3** for the last 30 days (including
today). **Esc** returns home.

- **Overview:** activity heatmap colored by earned XP, new words, word practices,
  review practices, active days, goal days, daily average, and streaks.
- **Words:** your current HSK 1 collection by textbook lesson, with new words in the selected
  period.
- **Review:** review practices in the selected period, current overdue/due words,
  and scheduled reviews for the next seven days. The forecast excludes today's
  backlog and reflects the current schedule, not a prediction of future activity.

The all-time heatmap displays the latest year; narrow terminals show fewer weeks.
Its date caption states the actual visible span, while totals use the selected
period. Current streak is always measured as of today; best streak is within the
selected period. Each word counts once per day in practice totals. Daily averages
include inactive days, starting no earlier than the profile's creation date.
The app does not infer study time, session counts, or accuracy from missing data.

Analytics uses existing local progress. It needs no network access, and viewing it
does not award XP or mark words learned. When piped, the command prints an
all-time text summary instead of opening the interactive dashboard.

## Commands and local data

```sh
cargo run -- today                 # Print the daily word
cargo run -- learn                 # Self-mark the daily word without a quiz
cargo run -- review                # Interactive review challenges
cargo run -- stats                 # Course size, XP, streak, due reviews
cargo run -- --data-dir ./my-data  # Use a separate learning profile
cargo install --path . --locked   # Install the xiuliu executable
```

Without a command, the app opens the interface in a terminal and prints today's
word when piped. Local data is stored under `$XDG_DATA_HOME/1day1word`,
`$LOCALAPPDATA/1day1word`, or `~/.local/share/1day1word`, in that order. The first two
variables must be absolute paths. `--data-dir PATH` overrides the location.
The original `1day1word` directory is retained so existing profiles keep their
learned words, XP, analytics, and downloaded vocabulary after the rename to Xiuliu.

`progress.json` stores learned words, XP, dates, and review schedules. Back it up when moving profiles. Old `vocabulary.json` update caches are ignored by the HSK 1-only course.
Old 30-word progress migrates automatically, with the original saved as
`progress.v1.backup.json`. Previously learned words and study dates are preserved.

A session lock prevents concurrent writes. After a forced termination, remove
`session.lock` from the data directory only once no session is running.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release --locked
```

`src/course.rs` handles vocabulary and updates; `src/progress.rs` owns persistence,
migration, XP, and review scheduling; `src/lesson.rs` generates challenges;
`src/ui.rs` renders the interface, and `src/audio.rs` handles local speech and chimes. `src/main.rs` handles CLI commands.

Tests exercise full lesson completion, free retries, rewards, calendar behavior,
migration, imports, terminal layouts, and CLI persistence. Data is embedded at
build time; rebuild after editing `assets/`. Keep the original `words.json` order
intact because it identifies entries when migrating old progress.

## License

Application code is covered by the [MIT license](LICENSE). Bundled vocabulary
has separate MIT and CC-BY-SA-4.0 terms; retain the attribution and notices in
[assets/NOTICE.md](assets/NOTICE.md) when redistributing it.
