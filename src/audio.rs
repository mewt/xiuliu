use crate::mimo;
use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

pub const CONTROLS: &str = "s listen · S slow · a sentence · m mute";

pub enum Cue {
    Correct,
    TryAgain,
    Complete,
}

pub struct Audio {
    voice: Option<String>,
    muted: bool,
    child: Option<Child>,
    notice: String,
    mimo: Option<mimo::Worker>,
    pending: Option<(u64, String, bool)>,
    temporary: Option<PathBuf>,
}

fn choose_voice(list: &str, traditional: bool) -> Option<String> {
    let locales = if traditional {
        ["zh_TW", "zh_CN"]
    } else {
        ["zh_CN", "zh_TW"]
    };
    for locale in locales {
        let voices: Vec<_> = list
            .lines()
            .filter_map(|line| {
                let (description, _) = line.split_once('#')?;
                let description = description.trim_end();
                description
                    .strip_suffix(locale)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
            })
            .collect();
        let preferred = if locale == "zh_TW" {
            "Meijia"
        } else {
            "Tingting"
        };
        if let Some(voice) = voices
            .iter()
            .find(|&&v| v == preferred)
            .or_else(|| voices.first())
        {
            return Some((*voice).to_owned());
        }
    }
    None
}

impl Audio {
    pub fn new(traditional: bool, muted: bool) -> Self {
        let voice = if cfg!(target_os = "macos") {
            Command::new("/usr/bin/say")
                .args(["-v", "?"])
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .ok()
                .filter(|output| output.status.success())
                .and_then(|output| {
                    choose_voice(&String::from_utf8_lossy(&output.stdout), traditional)
                })
        } else {
            None
        };
        Self {
            voice,
            muted,
            child: None,
            notice: String::new(),
            mimo: None,
            pending: None,
            temporary: None,
        }
    }
    pub fn with_mimo(mut self, config: mimo::Config) -> io::Result<Self> {
        if !cfg!(target_os = "macos") {
            return Err(io::Error::other(
                "MiMo audio playback currently requires macOS.",
            ));
        }
        self.mimo = Some(mimo::Worker::new(config));
        Ok(self)
    }
    #[cfg(test)]
    pub fn silent() -> Self {
        Self {
            voice: None,
            muted: true,
            child: None,
            notice: String::new(),
            mimo: None,
            pending: None,
            temporary: None,
        }
    }
    pub fn status(&self) -> &str {
        if !self.notice.is_empty() {
            &self.notice
        } else if self.muted {
            "Sound muted · m to enable"
        } else if self.mimo.is_some() {
            "MiMo · s listen · S slow · a sentence · m mute"
        } else if self.voice.is_none() {
            "Audio unavailable · s for details · m mute"
        } else {
            CONTROLS
        }
    }
    fn unavailable() -> &'static str {
        if cfg!(target_os = "macos") {
            "Install a Mandarin system voice in macOS settings."
        } else {
            "Offline sound currently requires macOS."
        }
    }
    pub fn toggle(&mut self) {
        self.stop();
        self.muted = !self.muted;
        self.notice.clear();
    }
    pub fn stop(&mut self) {
        if let Some(worker) = &self.mimo {
            worker.cancel();
        }
        self.pending = None;
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.remove_temporary();
    }
    fn remove_temporary(&mut self) {
        if let Some(path) = self.temporary.take() {
            let _ = fs::remove_file(path);
        }
    }
    pub fn message(&mut self, message: &str) {
        self.notice = message.to_owned();
    }
    pub fn speak(&mut self, text: &str, pinyin: &str, slow: bool) {
        self.stop();
        self.notice.clear();
        if self.muted {
            return;
        }
        if text.trim().is_empty() {
            self.notice = "This word has no example sentence yet.".into();
            return;
        }
        if let Some(worker) = &self.mimo {
            if let Some(id) = worker.speak(text, pinyin, slow) {
                self.pending = Some((id, text.into(), slow));
                self.notice = "MiMo is preparing audio · m mute".into();
            } else {
                self.fallback(text, slow);
            }
            return;
        }
        self.speak_local(text, slow);
    }
    fn speak_local(&mut self, text: &str, slow: bool) {
        let Some(voice) = self.voice.as_deref() else {
            self.notice = Self::unavailable().into();
            return;
        };
        match spawn_speech(voice, text, slow) {
            Ok(child) => self.child = Some(child),
            Err(_) => {
                self.notice =
                    "Could not play speech. Check your system voice and audio output.".into()
            }
        }
    }
    fn fallback(&mut self, text: &str, slow: bool) {
        self.speak_local(text, slow);
        self.notice = if self.child.is_some() {
            "MiMo unavailable · using offline speech".into()
        } else {
            "MiMo unavailable; no offline Mandarin voice is ready.".into()
        };
    }
    fn play_wav(&mut self, bytes: &[u8]) -> io::Result<()> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "xiuliu-audio-{}-{}.wav",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path)?;
        self.temporary = Some(path.clone());
        file.write_all(bytes)?;
        drop(file);
        self.child = Some(
            Command::new("/usr/bin/afplay")
                .arg(path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?,
        );
        Ok(())
    }
    pub fn cue(&mut self, cue: Cue) {
        if self.muted || !cfg!(target_os = "macos") {
            return;
        }
        self.stop();
        self.notice.clear();
        let path = match cue {
            Cue::Correct => "/System/Library/Sounds/Pop.aiff",
            Cue::TryAgain => "/System/Library/Sounds/Tink.aiff",
            Cue::Complete => "/System/Library/Sounds/Glass.aiff",
        };
        match Command::new("/usr/bin/afplay")
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => self.child = Some(child),
            Err(_) => self.notice = "Could not play sound. Check your audio output.".into(),
        }
    }
    fn poll_mimo(&mut self) {
        while let Some(reply) = self
            .mimo
            .as_ref()
            .and_then(|worker| worker.replies.try_recv().ok())
        {
            if !self
                .pending
                .as_ref()
                .is_some_and(|(id, _, _)| *id == reply.id)
            {
                continue;
            }
            let (_, text, slow) = self.pending.take().unwrap();
            self.notice.clear();
            match reply.audio {
                Ok(bytes) if self.play_wav(&bytes).is_ok() => {}
                _ => {
                    self.remove_temporary();
                    self.fallback(&text, slow);
                }
            }
        }
    }
    pub fn poll(&mut self) {
        self.poll_mimo();
        if let Some(child) = &mut self.child {
            match child.try_wait() {
                Ok(Some(status)) => {
                    self.child = None;
                    self.remove_temporary();
                    if !status.success() {
                        self.notice = "Audio failed. Check your system voice and output.".into();
                    }
                }
                Err(_) => {
                    self.stop();
                    self.notice = "Could not check audio playback.".into();
                }
                Ok(None) => {}
            }
        }
    }
    pub fn wait(&mut self) -> io::Result<()> {
        while self.pending.is_some() {
            self.poll_mimo();
            std::thread::sleep(Duration::from_millis(25));
        }
        let Some(mut child) = self.child.take() else {
            return Err(io::Error::other(if self.muted {
                "Sound is muted. Omit --mute to listen."
            } else if !self.notice.is_empty() {
                &self.notice
            } else {
                Self::unavailable()
            }));
        };
        let result = child.wait();
        self.remove_temporary();
        if result?.success() {
            Ok(())
        } else {
            Err(io::Error::other(
                "Speech failed. Check your system voice and audio output.",
            ))
        }
    }
}
impl Drop for Audio {
    fn drop(&mut self) {
        self.stop();
    }
}

fn spawn_speech(voice: &str, text: &str, slow: bool) -> io::Result<Child> {
    // Text goes through stdin, never through a shell or command-line options.
    let mut child = Command::new("/usr/bin/say")
        .args(["-v", voice, "-r", if slow { "100" } else { "170" }])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let result = child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("Speech input unavailable"))?
        .write_all(text.as_bytes());
    if let Err(error) = result {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    Ok(child)
}

#[cfg(test)]
mod tests {
    use super::*;
    const VOICES: &str = "Sinji zh_HK # Cantonese\nEddy (Chinese (China mainland)) zh_CN # Mandarin\nTingting zh_CN # Mandarin\nMeijia zh_TW # Mandarin\n";
    #[test]
    fn selects_mandarin_locale_and_handles_multiword_voice_names() {
        assert_eq!(choose_voice(VOICES, false).as_deref(), Some("Tingting"));
        assert_eq!(choose_voice(VOICES, true).as_deref(), Some("Meijia"));
        assert_eq!(
            choose_voice("Eddy (Chinese (Taiwan)) zh_TW # hi", false).as_deref(),
            Some("Eddy (Chinese (Taiwan))")
        );
        assert!(choose_voice("Sinji zh_HK # hi", false).is_none());
    }
    #[test]
    fn mute_prevents_speech_and_cues_without_spawning_processes() {
        let mut audio = Audio::silent();
        audio.speak("你好", "nǐ hǎo", false);
        audio.cue(Cue::Complete);
        assert!(audio.child.is_none());
        assert!(audio.status().contains("muted"));
        audio.toggle();
        audio.speak("你好", "nǐ hǎo", true);
        assert!(audio.child.is_none());
        assert!(!audio.notice.is_empty());
    }
}
