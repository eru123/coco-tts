use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::PathBuf;

const HF_BASE: &str = "https://huggingface.co/rhasspy/piper-voices/resolve/main";

/// A downloadable Piper voice, keyed by the CLI language code.
pub struct VoiceSpec {
    pub file: &'static str,
    pub hf_dir: &'static str,
    pub fallback_note: Option<&'static str>,
    pub unsupported: Option<&'static str>,
    /// Phonemize with this espeak-ng voice instead of the model's own; used
    /// to read one language through a phonetically closer voice.
    pub espeak_override: Option<&'static str>,
}

static EN: VoiceSpec = VoiceSpec {
    file: "en_US-lessac-medium.onnx",
    hf_dir: "en/en_US/lessac/medium/en_US-lessac-medium",
    fallback_note: None,
    unsupported: None,
    espeak_override: None,
};

// No Filipino Piper voice exists upstream; Tagalog text is read through the
// English model, phonemized by espeak-ng's Indonesian voice (a close
// Austronesian relative: pure vowels, similar stress).
static EN_FIL_FALLBACK: VoiceSpec = VoiceSpec {
    file: EN.file,
    hf_dir: EN.hf_dir,
    fallback_note: Some("no Filipino voice is available yet; using the en_US lessac voice as fallback"),
    unsupported: None,
    espeak_override: Some("id"),
};

static JA: VoiceSpec = VoiceSpec {
    file: "ja_JP-hi_fi_captain-medium.onnx",
    hf_dir: "ja/ja_JP/hi_fi_captain/medium/ja_JP-hi_fi_captain-medium",
    fallback_note: None,
    unsupported: Some("Japanese voices require OpenJTalk phonemization, which is not wired up yet"),
    espeak_override: None,
};

static KO: VoiceSpec = VoiceSpec {
    file: "ko_KR-kss-medium.onnx",
    hf_dir: "ko/ko_KR/kss/medium/ko_KR-kss-medium",
    fallback_note: None,
    unsupported: None,
    espeak_override: None,
};

static ZH: VoiceSpec = VoiceSpec {
    file: "zh_CN-huayan-medium.onnx",
    hf_dir: "zh/zh_CN/huayan/medium/zh_CN-huayan-medium",
    fallback_note: None,
    unsupported: None,
    espeak_override: None,
};

pub fn spec_for(lang_code: &str) -> Result<&'static VoiceSpec> {
    Ok(match lang_code {
        "en" => &EN,
        "fil" => &EN_FIL_FALLBACK,
        "jp" => &JA,
        "kr" => &KO,
        "cn" => &ZH,
        other => return Err(anyhow!("unknown language code '{other}'")),
    })
}

fn default_phoneme_type() -> String {
    "espeak".to_string()
}

#[derive(Deserialize)]
pub struct VoiceConfig {
    pub audio: AudioSection,
    pub espeak: EspeakSection,
    pub inference: InferenceSection,
    #[serde(rename = "phoneme_type", default = "default_phoneme_type")]
    pub phoneme_type: String,
    #[serde(rename = "phoneme_id_map")]
    pub phoneme_id_map: HashMap<String, Vec<i64>>,
}

#[derive(Deserialize)]
pub struct AudioSection {
    pub sample_rate: u32,
}

#[derive(Deserialize)]
pub struct EspeakSection {
    pub voice: String,
}

#[derive(Deserialize)]
pub struct InferenceSection {
    noise_scale: Option<f32>,
    length_scale: Option<f32>,
    noise_w: Option<f32>,
}

impl InferenceSection {
    pub fn noise_scale(&self) -> f32 {
        self.noise_scale.unwrap_or(0.667)
    }

    pub fn length_scale(&self) -> f32 {
        self.length_scale.unwrap_or(1.0)
    }

    pub fn noise_w(&self) -> f32 {
        self.noise_w.unwrap_or(0.8)
    }
}

pub struct LoadedVoice {
    pub model_path: PathBuf,
    pub config: VoiceConfig,
    pub espeak_override: Option<&'static str>,
}

impl LoadedVoice {
    /// The espeak-ng voice phonemization should use for this voice.
    pub fn espeak_voice(&self) -> &str {
        self.espeak_override.unwrap_or(&self.config.espeak.voice)
    }
}

pub fn models_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("coco-tts")
        .join("models")
}

/// Resolve (downloading on first use) the voice for a language code.
pub fn load(lang_code: &str) -> Result<LoadedVoice> {
    let spec = spec_for(lang_code)?;
    if let Some(reason) = spec.unsupported {
        return Err(anyhow!("{reason}"));
    }

    let model_path = models_dir().join(spec.file);
    let config_path = models_dir().join(format!("{}.json", spec.file));
    if !model_path.is_file() || !config_path.is_file() {
        download(spec, &model_path, &config_path)?;
    }

    let raw = fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read voice config {}", config_path.display()))?;
    let config: VoiceConfig = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse voice config {}", config_path.display()))?;
    Ok(LoadedVoice { model_path, config, espeak_override: spec.espeak_override })
}

fn download(spec: &VoiceSpec, model_path: &PathBuf, config_path: &PathBuf) -> Result<()> {
    fs::create_dir_all(models_dir()).context("failed to create the models directory")?;
    for (ext, dest) in [("onnx", model_path), ("onnx.json", config_path)] {
        let url = format!("{HF_BASE}/{}.{}", spec.hf_dir, ext);
        println!("[coco-tts] downloading {} ...", dest.file_name().unwrap().to_string_lossy());
        let response = ureq::get(&url)
            .call()
            .with_context(|| format!("download failed for {url}"))?;
        let mut bytes = Vec::new();
        response
            .into_reader()
            .read_to_end(&mut bytes)
            .with_context(|| format!("download interrupted for {url}"))?;
        let partial = dest.with_extension("part");
        fs::write(&partial, &bytes).with_context(|| format!("failed to write {}", partial.display()))?;
        fs::rename(&partial, dest).with_context(|| format!("failed to finalize {}", dest.display()))?;
    }
    Ok(())
}
