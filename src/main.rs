use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "coco-tts", author, version, about = "Cross-platform local TTS CLI powered by Rust and Piper ONNX")]
struct Args {
    /// Default English speech input
    #[arg(index = 1)]
    default_text: Option<String>,

    /// Tagalog / Filipino text segment
    #[arg(long)]
    fil: Option<String>,

    /// English text segment
    #[arg(long)]
    en: Option<String>,

    /// Japanese text segment
    #[arg(long)]
    jp: Option<String>,

    /// Korean text segment
    #[arg(long)]
    kr: Option<String>,

    /// Chinese text segment
    #[arg(long)]
    cn: Option<String>,
}

enum Language {
    Filipino,
    English,
    Japanese,
    Korean,
    Chinese,
}

struct SpeechSegment {
    lang: Language,
    text: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let mut segments: Vec<SpeechSegment> = Vec::new();

    // Priority 1: Check language flag arguments
    if let Some(text) = args.fil {
        segments.push(SpeechSegment { lang: Language::Filipino, text });
    }
    if let Some(text) = args.en {
        segments.push(SpeechSegment { lang: Language::English, text });
    }
    if let Some(text) = args.jp {
        segments.push(SpeechSegment { lang: Language::Japanese, text });
    }
    if let Some(text) = args.kr {
        segments.push(SpeechSegment { lang: Language::Korean, text });
    }
    if let Some(text) = args.cn {
        segments.push(SpeechSegment { lang: Language::Chinese, text });
    }

    // Priority 2: Fallback to default positional English text
    if segments.is_empty() {
        if let Some(text) = args.default_text {
            segments.push(SpeechSegment { lang: Language::English, text });
        } else {
            eprintln!("Error: No text provided. Run `coco-tts --help` for usage.");
            std::process::exit(1);
        }
    }

    // Initialize Audio Sink (rodio)
    let (_stream, stream_handle) = rodio::OutputStream::try_default()?;
    let sink = rodio::Sink::try_new(&stream_handle)?;

    // Process each speech segment through ONNX runtime and append to audio sink
    for segment in segments {
        let model_path = get_model_path(&segment.lang)?;
        println!("[coco-tts] Synthesizing {:?} text: \"{}\"", get_lang_code(&segment.lang), segment.text);

        // TODO: Pass phonemized text through ONNX model using `ort` crate
        // let audio_samples = synthesize_onnx(&model_path, &segment.text)?;
        // sink.append(rodio::buffer::SamplesBuffer::new(1, 22050, audio_samples));
    }

    sink.sleep_until_end();
    Ok(())
}

fn get_lang_code(lang: &Language) -> &'static str {
    match lang {
        Language::Filipino => "fil",
        Language::English => "en",
        Language::Japanese => "jp",
        Language::Korean => "kr",
        Language::Chinese => "cn",
    }
}

fn get_model_path(lang: &Language) -> anyhow::Result<PathBuf> {
    let base_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("coco-tts")
        .join("models");

    let model_name = match lang {
        Language::Filipino => "fil_PH-mms-medium.onnx",
        Language::English => "en_US-lessac-medium.onnx",
        Language::Japanese => "ja_JP-lessac-medium.onnx",
        Language::Korean => "ko_KR-vits-medium.onnx",
        Language::Chinese => "zh_CN-huayan-medium.onnx",
    };

    Ok(base_dir.join(model_name))
}
