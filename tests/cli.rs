use chrono::{Days, Local};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Sandbox(PathBuf);
impl Sandbox {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "xiuliu-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }

    fn run(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_xiuliu"))
            .arg("--data-dir")
            .arg(&self.0)
            .args(arguments)
            .output()
            .unwrap()
    }

    fn seed(&self, content: &str) {
        fs::create_dir_all(&self.0).unwrap();
        fs::write(self.0.join("progress.json"), content).unwrap();
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn first_run_prints_a_word_and_learning_is_persistent_and_idempotent() {
    let sandbox = Sandbox::new();
    let today = sandbox.run(&["today"]);
    assert!(today.status.success());
    let today_text = String::from_utf8_lossy(&today.stdout);
    assert!(today_text.contains("你  nǐ"));
    assert!(today_text.contains("Lesson 1/15 · Word 1/6"));
    let stats = sandbox.run(&["stats"]);
    assert!(String::from_utf8_lossy(&stats.stdout).contains("0 / "));
    for _ in 0..2 {
        assert!(sandbox.run(&["learn"]).status.success());
    }
    let stats = sandbox.run(&["stats"]);
    let output = String::from_utf8_lossy(&stats.stdout);
    assert!(output.contains("1 / "));
    assert!(output.contains("1-day streak"));
    assert!(output.contains("1 study days"));
    assert!(!sandbox.0.join("session.lock").exists());
}

#[test]
fn traditional_mode_and_calendar_rollover_work_with_existing_progress() {
    let sandbox = Sandbox::new();
    let yesterday = Local::now()
        .date_naive()
        .checked_sub_days(Days::new(1))
        .unwrap();
    sandbox.seed(&format!(
        r#"{{"started_on":"{yesterday}","completed":{{}}}}"#
    ));
    let output = sandbox.run(&["today", "--traditional"]);
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("謝謝"));
    assert!(text.contains("Lesson 2/15 · Word 1/4"));
}

#[test]
fn invalid_progress_is_reported_and_never_overwritten() {
    for content in [
        "broken json".to_owned(),
        format!(
            r#"{{"started_on":"{}","completed":{{"{}":999}}}}"#,
            Local::now().date_naive(),
            Local::now().date_naive()
        ),
    ] {
        let sandbox = Sandbox::new();
        sandbox.seed(&content);
        assert!(!sandbox.run(&["learn"]).status.success());
        assert_eq!(
            fs::read_to_string(sandbox.0.join("progress.json")).unwrap(),
            content
        );
        assert!(!sandbox.0.join("session.lock").exists());
    }
}

#[test]
fn help_bad_arguments_and_nonterminal_review_do_not_create_progress() {
    let sandbox = Sandbox::new();
    assert!(sandbox.run(&["--help"]).status.success());
    assert!(!sandbox.run(&["unknown"]).status.success());
    assert!(!sandbox.run(&["review"]).status.success());
    assert!(!sandbox.0.exists());
}

#[test]
fn an_existing_session_lock_prevents_concurrent_writes() {
    let sandbox = Sandbox::new();
    fs::create_dir_all(&sandbox.0).unwrap();
    fs::write(sandbox.0.join("session.lock"), "another process").unwrap();
    let output = sandbox.run(&["learn"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Another session may be running"));
    assert!(!sandbox.0.join("progress.json").exists());
    assert!(sandbox.0.join("session.lock").exists());
}

#[test]
fn migration_keeps_a_backup_and_preserves_legacy_completions() {
    let sandbox = Sandbox::new();
    let today = Local::now().date_naive();
    let legacy = format!(r#"{{"started_on":"{today}","completed":{{"{today}":0}}}}"#);
    sandbox.seed(&legacy);
    assert!(sandbox.run(&["stats"]).status.success());
    assert_eq!(
        fs::read_to_string(sandbox.0.join("progress.v1.backup.json")).unwrap(),
        legacy
    );
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(sandbox.0.join("progress.json")).unwrap()).unwrap();
    assert_eq!(value["version"], 2);
    assert!(value["learned"].get("你好").is_some());
    assert!(sandbox.run(&["learn"]).status.success());
    let migrated_stats = sandbox.run(&["stats"]);
    let migrated_text = String::from_utf8_lossy(&migrated_stats.stdout);
    assert!(migrated_text.contains("1 / 166 words learned"));
    assert!(migrated_text.contains("0 XP"));
}

#[test]
fn speech_options_are_scoped_and_muted_speech_does_not_award_progress() {
    let sandbox = Sandbox::new();
    assert!(!sandbox.run(&["today", "--slow"]).status.success());
    assert!(!sandbox.run(&["--sentence"]).status.success());
    assert!(!sandbox.0.exists());
    let output = sandbox.run(&["speak", "--mute"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("muted"));
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(sandbox.0.join("progress.json")).unwrap()).unwrap();
    assert!(state["learned"].as_object().unwrap().is_empty());
}

#[test]
fn tts_provider_is_explicit_and_local_mode_does_not_read_mimo_config() {
    let sandbox = Sandbox::new();
    assert!(!sandbox.run(&["--tts", "unknown"]).status.success());
    assert!(!sandbox.run(&["--tts"]).status.success());
    assert!(!sandbox.run(&["--tts", "mimo", "--voice"]).status.success());
    assert!(
        !sandbox
            .run(&["--tts", "mimo", "--voice", "unknown"])
            .status
            .success()
    );
    assert!(!sandbox.run(&["--voice", "male"]).status.success());
    assert!(!sandbox.0.exists());
    fs::create_dir_all(&sandbox.0).unwrap();
    fs::write(sandbox.0.join(".env"), "MIMO_API_KEY='broken-secret").unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_xiuliu"))
            .current_dir(&sandbox.0)
            .arg("--data-dir")
            .arg(&sandbox.0)
            .args(args)
            .output()
            .unwrap()
    };
    assert!(run(&["today", "--tts", "local"]).status.success());
    for voice in ["female", "male", "moli", "baihua"] {
        let muted = run(&["speak", "--tts", "mimo", "--voice", voice, "--mute"]);
        assert!(String::from_utf8_lossy(&muted.stderr).contains("muted"));
    }
    let muted = run(&["speak", "--tts", "mimo", "--mute"]);
    assert!(String::from_utf8_lossy(&muted.stderr).contains("muted"));
    let active = run(&["speak", "--tts", "mimo"]);
    assert!(!active.status.success());
    let error = String::from_utf8_lossy(&active.stderr);
    assert!(error.contains("Invalid quoted MiMo setting"));
    assert!(!error.contains("broken-secret"));
}

#[test]
fn analytics_summarizes_saved_history_without_changing_it() {
    let sandbox = Sandbox::new();
    assert!(sandbox.run(&["learn"]).status.success());
    let before = fs::read(sandbox.0.join("progress.json")).unwrap();
    let output = sandbox.run(&["analytics"]);
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("XIULIU ANALYTICS"));
    assert!(text.contains("20 XP"));
    assert!(text.contains("1 word practices (0 reviews)"));
    assert_eq!(fs::read(sandbox.0.join("progress.json")).unwrap(), before);
}

#[test]
fn analytics_for_a_new_profile_does_not_create_learning_history() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&["analytics"]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("0 active days"));
    assert!(!sandbox.0.join("progress.json").exists());
}
