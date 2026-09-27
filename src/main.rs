use anyhow::{anyhow, Context, Result};
use rodio::cpal::traits::{DeviceTrait as _, HostTrait as _};
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

    /// Play through a specific audio device (see --list-devices)
    #[arg(long, value_name = "NAME")]
    device: Option<String>,

    /// List available audio output devices and exit
    #[arg(long)]
    list_devices: bool,
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

    if args.list_devices {
        let default = rodio::cpal::default_host().default_output_device().and_then(|d| d.name().ok());
        for device in rodio::cpal::default_host().output_devices().ok().into_iter().flatten() {
            let name = device.name().unwrap_or_else(|_| "<unnamed>".to_string());
            let marker = if Some(&name) == default.as_ref() { " (default)" } else { "" };
            println!("{name}{marker}");
        }
        return Ok(());
    }

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

    let mut wav = None;
    let mut rendered: Vec<(u32, Vec<i16>)> = Vec::new();
    let mut spoke_any = false;

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
        let pcm: Vec<i16> = audio
            .samples
            .iter()
            .map(|sample| (sample.clamp(-1.0, 1.0) * 32767.0) as i16)
            .collect();
        rendered.push((audio.sample_rate, pcm));

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

    if !args.no_play {
        if let Err(e) = play(&rendered, args.device.as_deref()) {
            if args.out.is_some() {
                eprintln!("[coco-tts] {e:#}; the WAV file was still written");
            } else {
                return Err(e);
            }
        }
    }
    Ok(())
}

/// Append the rendered audio and block until the sink drains. Returns false
/// if the device never consumes it within the timeout.
fn drain(sink: &rodio::Sink, timeout: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while !sink.empty() {
        if std::time::Instant::now() > deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    true
}

/// Play on one device. A third of a second of silence is appended first as a
/// liveness probe: some PipeWire-backed ALSA defaults open successfully but
/// never run their callback thread, which would otherwise stall silently.
fn play_on(device: &rodio::Device, rendered: &[(u32, Vec<i16>)], seconds: f32) -> bool {
    let Ok((stream, handle)) = rodio::OutputStream::try_from_device(device) else {
        return false;
    };
    let Ok(sink) = rodio::Sink::try_new(&handle) else {
        return false;
    };

    sink.append(rodio::buffer::SamplesBuffer::new(1, 22_050, vec![0i16; 7_350]));
    if !drain(&sink, std::time::Duration::from_secs(3)) {
        return false;
    }

    for (rate, pcm) in rendered {
        sink.append(rodio::buffer::SamplesBuffer::new(1, *rate, pcm.clone()));
    }
    if !drain(&sink, std::time::Duration::from_secs(seconds as u64 + 10)) {
        eprintln!("[coco-tts] playback stalled near the end; dropping the rest");
    }
    drop(sink);
    drop(stream);
    true
}

/// Play the rendered segments, falling back across output devices until one
/// actually consumes audio.
fn play(rendered: &[(u32, Vec<i16>)], device_name: Option<&str>) -> anyhow::Result<()> {
    let seconds: f32 = rendered
        .iter()
        .map(|(rate, pcm)| pcm.len() as f32 / *rate as f32)
        .sum();

    if let Some(wanted) = device_name {
        let device = rodio::cpal::default_host().output_devices().ok().into_iter().flatten()
            .find(|device| device.name().ok().as_deref() == Some(wanted))
            .ok_or_else(|| anyhow!("no audio output device named '{wanted}'"))?;
        if !play_on(&device, rendered, seconds) {
            return Err(anyhow!("audio device '{wanted}' did not play the audio"));
        }
        return Ok(());
    }

    // The default gets first shot without touching the device list: probing
    // every device spews ALSA/JACK diagnostics on stderr.
    if let Some(default) = rodio::cpal::default_host().default_output_device() {
        if play_on(&default, rendered, seconds) {
            return Ok(());
        }
        eprintln!(
            "[coco-tts] default audio device '{}' is not responding; trying other devices",
            default.name().unwrap_or_else(|_| "<unnamed>".to_string())
        );
    }

    for device in rodio::cpal::default_host().output_devices().ok().into_iter().flatten() {
        let name = device.name().ok();
        if name.is_none() || name.as_deref() == default_name().as_deref() {
            continue;
        }
        if play_on(&device, rendered, seconds) {
            return Ok(());
        }
        eprintln!(
            "[coco-tts] audio device '{}' is not responding; trying the next one",
            device.name().unwrap_or_else(|_| "<unnamed>".to_string())
        );
    }
    Err(anyhow!(
        "no working audio output device; pass --out <file.wav> or --no-play"
    ))
}

fn default_name() -> Option<String> {
    rodio::cpal::default_host()
        .default_output_device()
        .and_then(|device| device.name().ok())
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
