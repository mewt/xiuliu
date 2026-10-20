use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs,
    hash::{Hash, Hasher},
    io::{self, Read},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    time::Duration,
};

// Deliberately not Debug: this object contains a credential.
pub struct Config {
    key: String,
    base_url: String,
    model: String,
    voice: String,
    cache_dir: Option<PathBuf>,
}

pub fn voice_id(value: &str) -> io::Result<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "female" | "bingtang" | "冰糖" => Ok("冰糖"),
        "moli" | "茉莉" => Ok("茉莉"),
        "male" | "suda" | "苏打" => Ok("苏打"),
        "baihua" | "白桦" => Ok("白桦"),
        _ => Err(io::Error::other(
            "Choose a MiMo Mandarin voice: female, male, bingtang, moli, suda, or baihua.",
        )),
    }
}

fn dotenv(text: &str) -> io::Result<HashMap<String, String>> {
    let mut values = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if !matches!(
            key,
            "MIMO_API_KEY" | "MIMO_BASE_URL" | "MIMO_MODEL" | "MIMO_VOICE"
        ) {
            continue;
        }
        let value = value.trim();
        let value = if value.starts_with(['\'', '"']) {
            let quote = value.chars().next().unwrap();
            if value.len() < 2 || !value.ends_with(quote) {
                return Err(io::Error::other("Invalid quoted MiMo setting in .env."));
            }
            &value[1..value.len() - 1]
        } else {
            value
                .split_once(" #")
                .map_or(value, |(value, _)| value)
                .trim()
        };
        // Parse data only. Never execute shell substitutions or mutate process env.
        values.insert(key.into(), value.into());
    }
    Ok(values)
}

impl Config {
    pub fn load(path: &Path, voice_override: Option<&str>) -> io::Result<Self> {
        let values = match fs::read_to_string(path) {
            Ok(text) => dotenv(&text)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => HashMap::new(),
            Err(_) => return Err(io::Error::other("Could not read MiMo .env configuration.")),
        };
        let get = |key: &str, default: &str| {
            std::env::var(key)
                .ok()
                .or_else(|| values.get(key).cloned())
                .unwrap_or_else(|| default.into())
        };
        let config = Self {
            key: get("MIMO_API_KEY", ""),
            base_url: get("MIMO_BASE_URL", "https://token-plan-sgp.xiaomimimo.com/v1")
                .trim_end_matches('/')
                .into(),
            model: get("MIMO_MODEL", "mimo-v2.5-tts"),
            voice: voice_id(voice_override.unwrap_or(&get("MIMO_VOICE", "冰糖")))?.into(),
            cache_dir: None,
        };
        if config.key.trim().is_empty() {
            return Err(io::Error::other(
                "Set MIMO_API_KEY in .env or your environment to use --tts mimo.",
            ));
        }
        if !matches!(
            config.base_url.as_str(),
            "https://api.xiaomimimo.com/v1"
                | "https://token-plan-sgp.xiaomimimo.com/v1"
                | "https://token-plan-cn.xiaomimimo.com/v1"
                | "https://token-plan-ams.xiaomimimo.com/v1"
        ) {
            return Err(io::Error::other(
                "MIMO_BASE_URL must be an official Xiaomi MiMo HTTPS endpoint.",
            ));
        }
        if config.model != "mimo-v2.5-tts" {
            return Err(io::Error::other(
                "This speech provider supports MIMO_MODEL=mimo-v2.5-tts.",
            ));
        }
        Ok(config)
    }

    /// Persist generated audio here so each clip is fetched once, then reused
    /// across sessions and offline.
    pub fn with_cache_dir(mut self, dir: PathBuf) -> Self {
        self.cache_dir = Some(dir);
        self
    }

    fn request(&self, text: &str, pinyin: &str, slow: bool) -> Value {
        let pace = if slow {
            "slowly for a beginner"
        } else {
            "at a natural teaching pace"
        };
        let mut instruction = format!(
            "Read only the provided Chinese text in standard Mandarin, {pace}. Do not add words, repeat the text, or read instructions aloud."
        );
        if !pinyin.trim().is_empty() {
            instruction.push_str(&format!(
                " The intended pronunciation in Hanyu Pinyin is: {pinyin}. Use this to disambiguate the Chinese characters and their lexical tones, with natural Mandarin tone sandhi. Do not spell out or read the pinyin letters."
            ));
        }
        json!({
            "model": self.model,
            "messages": [
                {"role": "user", "content": instruction},
                {"role": "assistant", "content": text}
            ],
            "audio": {"format": "wav", "voice": self.voice}
        })
    }

    fn fetch(&self, body: &str) -> io::Result<Vec<u8>> {
        let response = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(30))
            .redirects(0)
            .build()
            .post(&format!("{}/chat/completions", self.base_url))
            .set("Authorization", &format!("Bearer {}", self.key))
            .set("Content-Type", "application/json")
            .send_string(body)
            .map_err(|error| match error {
                ureq::Error::Status(status, _) => {
                    io::Error::other(format!("MiMo returned HTTP {status}."))
                }
                _ => io::Error::other("MiMo connection failed or timed out."),
            })?;
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(16_000_001)
            .read_to_end(&mut bytes)
            .map_err(|_| io::Error::other("Could not read MiMo audio response."))?;
        if bytes.len() > 16_000_000 {
            return Err(io::Error::other("MiMo audio response is too large."));
        }
        decode_audio(&bytes)
    }
}

fn decode_audio(bytes: &[u8]) -> io::Result<Vec<u8>> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| io::Error::other("Invalid MiMo audio response."))?;
    let data = value
        .pointer("/choices/0/message/audio/data")
        .and_then(Value::as_str)
        .ok_or_else(|| io::Error::other("MiMo returned no audio."))?;
    let wav = STANDARD
        .decode(data)
        .map_err(|_| io::Error::other("Invalid MiMo audio encoding."))?;
    if wav.len() < 44 || &wav[..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return Err(io::Error::other("MiMo returned invalid WAV audio."));
    }
    Ok(wav)
}

pub struct Reply {
    pub id: u64,
    // Path to the cached WAV on disk; it is kept, not deleted after playback.
    pub audio: io::Result<PathBuf>,
}
pub struct Worker {
    requests: SyncSender<(u64, String, String, bool)>,
    pub replies: Receiver<Reply>,
    generation: Arc<AtomicU64>,
}

fn digest(body: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    body.hash(&mut hasher);
    // Include length so distinct bodies cannot alias on a 64-bit hash alone.
    format!("{:016x}{:x}", hasher.finish(), body.len())
}

// Reuse a saved clip when we have one; otherwise fetch it and persist it.
fn resolve(
    config: &Config,
    body: &str,
    cache: &mut HashMap<String, PathBuf>,
    fetch: &mut impl FnMut(&Config, &str) -> io::Result<Vec<u8>>,
) -> io::Result<PathBuf> {
    if let Some(path) = cache.get(body) {
        if path.exists() {
            return Ok(path.clone());
        }
    }
    let name = format!("mimo-{}.wav", digest(body));
    let path = match &config.cache_dir {
        Some(dir) => dir.join(name),
        None => std::env::temp_dir().join(format!("xiuliu-{name}")),
    };
    if path.exists() {
        cache.insert(body.to_owned(), path.clone());
        return Ok(path);
    }
    let bytes = fetch(config, body)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    // Write to a sibling file, then rename, so a reader never sees a partial clip.
    let partial = path.with_extension("part");
    fs::write(&partial, &bytes)?;
    fs::rename(&partial, &path)?;
    if cache.len() >= 256 {
        cache.clear();
    }
    cache.insert(body.to_owned(), path.clone());
    Ok(path)
}

impl Worker {
    pub fn new(config: Config) -> Self {
        Self::spawn(config, Config::fetch)
    }
    fn spawn(
        config: Config,
        mut fetch: impl FnMut(&Config, &str) -> io::Result<Vec<u8>> + Send + 'static,
    ) -> Self {
        let (requests, incoming) = mpsc::sync_channel::<(u64, String, String, bool)>(1);
        let (outgoing, replies) = mpsc::channel();
        let generation = Arc::new(AtomicU64::new(0));
        let current = generation.clone();
        std::thread::spawn(move || {
            // Maps a request to its saved clip so we neither refetch nor rehash it.
            let mut cache: HashMap<String, PathBuf> = HashMap::new();
            while let Ok((id, text, pinyin, slow)) = incoming.recv() {
                if current.load(Ordering::SeqCst) != id {
                    continue;
                }
                let body = config.request(&text, &pinyin, slow).to_string();
                let audio = resolve(&config, &body, &mut cache, &mut fetch);
                if current.load(Ordering::SeqCst) == id
                    && outgoing.send(Reply { id, audio }).is_err()
                {
                    break;
                }
            }
        });
        Self {
            requests,
            replies,
            generation,
        }
    }
    pub fn cancel(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }
    pub fn speak(&self, text: &str, pinyin: &str, slow: bool) -> Option<u64> {
        let id = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.requests
            .try_send((id, text.into(), pinyin.into(), slow))
            .ok()
            .map(|_| id)
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp_cache(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "xiuliu-test-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }
    #[test]
    fn listening_word_sends_ting_with_male_voice_and_keeps_hints_out_of_spoken_text() {
        let word = crate::course::bundled()
            .into_iter()
            .find(|word| word.simplified == "听")
            .unwrap();
        assert_eq!(word.pinyin, "tīng");
        assert_eq!(word.meaning, "to listen");
        let config = Config {
            key: "test-only".into(),
            base_url: "https://example.invalid".into(),
            model: "mimo-v2.5-tts".into(),
            voice: voice_id("male").unwrap().into(),
            cache_dir: Some(temp_cache("ting")),
        };
        let (sent, received) = mpsc::channel();
        let worker = Worker::spawn(config, move |_, body| {
            sent.send(serde_json::from_str::<Value>(body).unwrap())
                .unwrap();
            Ok(vec![1, 2, 3])
        });
        worker.speak(&word.simplified, &word.pinyin, false).unwrap();
        let request = received.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(request["audio"]["voice"], "苏打");
        assert_eq!(request["messages"][1]["content"], "听");
        assert!(
            request["messages"][0]["content"]
                .as_str()
                .unwrap()
                .contains("tīng")
        );
        worker.replies.recv_timeout(Duration::from_secs(2)).unwrap();
        // A different reading must not reuse audio cached for the same character.
        worker.speak("听", "yǐn", false).unwrap();
        let other = received.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(
            other["messages"][0]["content"]
                .as_str()
                .unwrap()
                .contains("yǐn")
        );
    }
    #[test]
    fn cancelled_requests_do_not_play_and_replays_use_the_session_cache() {
        let config = Config {
            key: "never-sent".into(),
            base_url: "https://example.invalid".into(),
            model: "mimo-v2.5-tts".into(),
            voice: "冰糖".into(),
            cache_dir: Some(temp_cache("cancel")),
        };
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let calls = Arc::new(AtomicU64::new(0));
        let counter = calls.clone();
        let worker = Worker::spawn(config, move |_, _| {
            counter.fetch_add(1, Ordering::SeqCst);
            started_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            Ok(vec![1, 2, 3])
        });
        worker.speak("你好", "nǐ hǎo", false).unwrap();
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        worker.cancel();
        let current = worker.speak("你好", "nǐ hǎo", false).unwrap();
        release_tx.send(()).unwrap();
        let reply = worker.replies.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(reply.id, current);
        assert_eq!(fs::read(reply.audio.unwrap()).unwrap(), vec![1, 2, 3]);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(worker.replies.try_recv().is_err());
    }
    #[test]
    fn dotenv_is_data_only_and_does_not_expose_secrets_in_errors() {
        let values = dotenv("export MIMO_API_KEY='test-secret'\nMIMO_VOICE=冰糖 # voice\nUNRELATED=x\nMIMO_MODEL=$(touch ignored)").unwrap();
        assert_eq!(values["MIMO_API_KEY"], "test-secret");
        assert_eq!(values["MIMO_VOICE"], "冰糖");
        assert_eq!(values["MIMO_MODEL"], "$(touch ignored)");
        assert!(!values.contains_key("UNRELATED"));
        assert!(
            !dotenv("MIMO_API_KEY='private")
                .unwrap_err()
                .to_string()
                .contains("private")
        );
    }
    #[test]
    fn request_preserves_text_and_selects_mandarin_voice() {
        let mut config = Config {
            key: "secret".into(),
            base_url: "https://example.invalid".into(),
            model: "mimo-v2.5-tts".into(),
            voice: "冰糖".into(),
            cache_dir: None,
        };
        let normal = config.request("你好！", "nǐ hǎo", false);
        assert_eq!(normal["messages"][1]["content"], "你好！");
        assert_eq!(normal["messages"][1]["role"], "assistant");
        assert_eq!(normal["audio"]["voice"], "冰糖");
        assert_ne!(normal, config.request("你好！", "nǐ hǎo", true));
        assert!(!normal.to_string().contains("secret"));
        config.voice = voice_id("male").unwrap().into();
        let male = config.request("你好！", "nǐ hǎo", false);
        assert_eq!(male["audio"]["voice"], "苏打");
        assert_ne!(normal, male);
        for (alias, expected) in [
            ("female", "冰糖"),
            ("BingTang", "冰糖"),
            ("moli", "茉莉"),
            ("suda", "苏打"),
            ("baihua", "白桦"),
            ("白桦", "白桦"),
        ] {
            assert_eq!(voice_id(alias).unwrap(), expected);
        }
        assert!(voice_id("unknown").is_err());
    }
    #[test]
    fn malformed_or_missing_audio_is_rejected() {
        assert!(decode_audio(b"not json").is_err());
        assert!(decode_audio(br#"{"choices":[]}"#).is_err());
        let mut wav = vec![0; 44];
        wav[..4].copy_from_slice(b"RIFF");
        wav[8..12].copy_from_slice(b"WAVE");
        let value = json!({"choices":[{"message":{"audio":{"data":STANDARD.encode(&wav)}}}]});
        assert_eq!(decode_audio(value.to_string().as_bytes()).unwrap(), wav);
        let bad = json!({"choices":[{"message":{"audio":{"data":STANDARD.encode(b"not wav")}}}]});
        assert!(decode_audio(bad.to_string().as_bytes()).is_err());
    }
}
