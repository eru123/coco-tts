# coco-tts

Cross-platform local text-to-speech (TTS) CLI powered by Rust and [Piper](https://github.com/rhasspy/piper) ONNX voice models. No cloud, no API keys — speech synthesis runs entirely on your machine.

## Features

- **Multi-language speech** — Filipino (`--fil`), English (`--en`), Japanese (`--jp`), Korean (`--kr`), and Chinese (`--cn`) segments, spoken in the order given.
- **Local-first** — ONNX Runtime inference via the `ort` crate; audio playback via `rodio` (CPAL / ALSA / CoreAudio / WASAPI).
- **Simple CLI** — positional text defaults to English; language flags give explicit control.

## Usage

```bash
# English (default)
coco-tts "Hello Master Jericho!"

# Filipino
coco-tts --fil "Kamusta ka boss Jericho!"

# Mixed languages, spoken sequentially
coco-tts --fil "Kamusta ka boss Jericho!" --en "All tasks completed"
```

## Setup

### 1. Build

Requires a recent Rust toolchain ([rustup](https://rustup.rs/)).

```bash
cargo build --release
```

> **Linux note:** `rodio` needs ALSA headers — install `libasound2-dev` (Debian/Ubuntu) before building.

### 2. Download voice models

Place Piper `.onnx` voice models (and their `.onnx.json` configs) in the models directory:

- Linux/macOS: `~/.local/share/coco-tts/models/` (macOS: `~/Library/Application Support/coco-tts/models/`)
- Windows: `%APPDATA%\coco-tts\models\`

Expected model files:

| Language | Model file |
| --- | --- |
| Filipino | `fil_PH-mms-medium.onnx` |
| English | `en_US-lessac-medium.onnx` |
| Japanese | `ja_JP-lessac-medium.onnx` |
| Korean | `ko_KR-vits-medium.onnx` |
| Chinese | `zh_CN-huayan-medium.onnx` |

Voice models are available from the [Piper voices collection](https://huggingface.co/rhasspy/piper-voices). Models are gitignored — download them separately.

## Status

Early scaffold. CLI parsing, segment ordering, model path resolution, and audio sink wiring are in place; ONNX synthesis (`ort`) is the next milestone.

## License

Private repository. All rights reserved.
