use anyhow::{anyhow, Context, Result};
use clap::Parser;
use std::path::PathBuf;

mod phonemize;
mod synthesize;
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

    /// Write the synthesized audio to a WAV file (in addition to playback)
    #[arg(long, value_name = "FILE")]
    out: Option<PathBuf>,

    /// Skip audio playback (useful on headless machines; pair with --out)
    #[arg(long)]
    no_play: bool,
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

fn main() -> Result<()> {
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

    // Playback is best effort so headless machines can still render to a file.
    let sink = if args.no_play {
        None
    } else {
        match rodio::OutputStream::try_default() {
            Ok((_stream, handle)) => Some(rodio::Sink::try_new(&handle)?),
            Err(_) => {
                if args.out.is_none() {
                    return Err(anyhow!(
                        "no audio output device found; pass --out <file.wav> or --no-play"
                    ));
                }
                eprintln!("[coco-tts] no audio output device; writing WAV only");
                None
            }
        }
    };

    let mut wav = None;
    let mut spoke_any = false;
    let mut spoken_seconds = 0.0_f32;

    for segment in &segments {
        let code = get_lang_code(&segment.lang);
        println!("[coco-tts] Synthesizing [{}] \"{}\"", code, segment.text);

        let voice = match voice::load(code) {
            Ok(voice) => voice,
            Err(e) => {
                eprintln!("[coco-tts] skipping {code} segment: {e:#}");
                continue;
            }
        };
        if let Some(note) = voice::spec_for(code).ok().and_then(|spec| spec.fallback_note) {
            eprintln!("[coco-tts] {note}");
        }

        let sentences = phonemize::phoneme_ids(&voice.config, &segment.text)?;
        let audio = synthesize::run(&voice, &sentences)?;
        spoke_any = true;
        spoken_seconds += audio.samples.len() as f32 / audio.sample_rate as f32;

        if let Some(sink) = &sink {
            let pcm: Vec<i16> = audio
                .samples
                .iter()
                .map(|sample| (sample.clamp(-1.0, 1.0) * 32767.0) as i16)
                .collect();
            sink.append(rodio::buffer::SamplesBuffer::new(1, audio.sample_rate, pcm));
        }

        if let Some(path) = &args.out {
            if wav.is_none() {
                let spec = hound::WavSpec {
                    channels: 1,
                    sample_rate: audio.sample_rate,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                };
                wav = Some(
                    hound::WavWriter::create(path, spec)
                        .with_context(|| format!("failed to create {}", path.display()))?,
                );
            }
            if let Some(writer) = wav.as_mut() {
                for sample in &audio.samples {
                    writer.write_sample((sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)?;
                }
            }
        }
    }

    if let Some(writer) = wav {
        writer.finalize()?;
    }
    if !spoke_any {
        return Err(anyhow!("no segments could be synthesized"));
    }
    if let Some(sink) = sink {
        // A broken or phantom audio device can stall the drain forever, so
        // cap the wait at the synthesized duration plus a generous margin.
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_secs(spoken_seconds as u64 + 15);
        while !sink.empty() {
            if std::time::Instant::now() > deadline {
                eprintln!("[coco-tts] playback stalled; exiting without draining the sink");
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
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
