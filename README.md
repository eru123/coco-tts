# coco-tts

Cross-platform local text-to-speech (TTS) CLI powered by Rust and [Piper](https://github.com/rhasspy/piper) ONNX voice models. No cloud, no API keys — phonemization (espeak-ng), inference (ONNX Runtime via `ort`), and playback (`rodio`) all run on your machine.

## Usage

```bash
# English (default)
coco-tts "Hello Master Jericho!"

# Filipino
coco-tts --fil "Kamusta ka boss Jericho!"

# Mixed languages, spoken sequentially
coco-tts --fil "Kamusta ka boss Jericho!" --en "All tasks completed"

# Korean and Chinese
coco-tts --kr "안녕하세요, 보스 제리코!" --cn "你好，世界！"

# Headless / scripted: skip playback, write a WAV instead
coco-tts "Hello Master Jericho!" --out hello.wav --no-play

# Pin playback to a specific device (list them first)
coco-tts --list-devices
coco-tts "Hello" --device pulse
```

Language segments always execute in flag order: `--fil`, `--en`, `--jp`, `--kr`, `--cn`. With no flags, positional text is spoken as English.

## Tweaking pronunciation

A user dictionary rewrites how words are spoken, without rebuilding: add lines to `~/.config/coco-tts/pronunciation.tsv` in the form `word<TAB>say it like this` (case-insensitive, whole-word, `#` starts a comment). Respellings use ordinary letters and hyphens and are applied before phonemization:

```bash
jericho	jeh-rih-koh
coco-tts	ko-ko tee tee ess
```

Preview what will be said without synthesizing:

```bash
coco-tts "Hello Master Jericho!" --print-phonemes
# [en] h|ə|l|ˈoʊ m|ˈæ|s|t|ɚ dʒ|ˈeɪ|ɹ|ˈɪ|k|ˈoʊ!
```

`--espeak-voice VOICE` overrides the phonemization accent for a run (e.g. `--espeak-voice en-gb` for British diphthongs). Since `--fil` has no Filipino model, it phonemizes through espeak-ng's Indonesian voice — pure vowels and stress patterns much closer to Tagalog — while synthesizing with the English model; per-word trouble cases can be corrected in the dictionary.

## Voices

Voices are resolved from the [piper-voices](https://huggingface.co/rhasspy/piper-voices) collection and downloaded automatically to the platform data dir on first use (Linux `~/.local/share/coco-tts/models/`, macOS `~/Library/Application Support/coco-tts/models/`, Windows `%APPDATA%\coco-tts\models\`).

| Flag | Voice | Status |
| --- | --- | --- |
| `--en` | `en_US-lessac-medium` | works |
| `--fil` | *(no upstream Filipino voice)* | falls back to the `en_US` voice with a warning — Tagalog's phonetic Latin script stays intelligible |
| `--jp` | `ja_JP-hi_fi_captain-medium` | pending: needs OpenJTalk phonemization, segment is skipped with a notice |
| `--kr` | `ko_KR-kss-medium` | works |
| `--cn` | `zh_CN-huayan-medium` | works |

Models are gitignored; swapping any voice is a one-line change to the table in `src/voice.rs`.

## Setup

Requires a Rust toolchain ([rustup](https://rustup.rs/)) and `espeak-ng` for phonemization.

```bash
# Debian/Ubuntu: rodio needs ALSA headers, phonemization needs espeak-ng
sudo apt install libasound2-dev espeak-ng

cargo build --release
```

## How it works

1. Text is split into sentences; each is phonemized with `espeak-ng --ipa` in the voice's language.
2. Phonemes map to the model's `phoneme_id_map` ids (whole symbol first, then per codepoint), framed with BOS/EOS and pause ids — the same contract the Piper VITS models were exported with.
3. `ort` runs the ONNX session (`input`, `input_lengths`, `scales`, `sid`) and the per-sentence audio is stitched with a 200 ms inter-sentence gap.
4. Audio plays through `rodio`, and `--out` additionally writes a 16-bit mono WAV (`hound`).

Playback probes the default output device with a short burst of silence first: on systems where the ALSA default routes into a suspended or cold PipeWire node, the stream can open successfully but never actually consume audio. If the probe stalls, playback automatically falls back through the other output devices (`pulse`, `pipewire`, raw `hw:`), so a flaky default never costs more than a few seconds. `--list-devices` shows what is available and `--device NAME` pins one explicitly.

## License

Private repository. All rights reserved.
