use clap::Parser;

mod voice;

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
        let code = get_lang_code(&segment.lang);
        println!("[coco-tts] Synthesizing [{}] \"{}\"", code, segment.text);

        if let Err(e) = voice::load(code) {
            eprintln!("[coco-tts] skipping {code} segment: {e:#}");
            continue;
        }
        if let Some(spec) = voice::spec_for(code).ok().filter(|s| s.fallback_note.is_some()) {
            if let Some(note) = spec.fallback_note {
                eprintln!("[coco-tts] {note}");
            }
        }

        // TODO: Pass phonemized text through ONNX model using `ort` crate
        // let audio_samples = synthesize(&loaded, &segment.text)?;
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
