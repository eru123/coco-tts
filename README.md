# coco-tts

tts for the terminal. rust + piper onnx voices, all local, no cloud, no keys.

## tldr

```bash
sudo apt install espeak-ng libasound2-dev   # linux, once
cargo build --release
coco-tts "Hello Master Jericho!"
```

first run pulls the voice (~60MB) into `~/.local/share/coco-tts/models/` and just works after that.

## usage

```bash
coco-tts "Hello"                      # english
coco-tts --fil "Kamusta boss!"        # tagalog
coco-tts --kr "안녕하세요" --cn "你好"   # korean, chinese
coco-tts "..." --out out.wav          # also write a wav
coco-tts "..." --no-play              # skip playback (headless boxes)
coco-tts "..." --print-phonemes       # show what it'll say, no audio
coco-tts --list-devices               # audio devices
coco-tts "..." --device pulse         # force one
coco-tts "..." --espeak-voice en-gb   # accent swap
```

segments run in flag order: fil, en, jp, kr, cn. no flags = positional text is english.

## voices

| flag | voice | state |
| --- | --- | --- |
| `--en` | en_US-lessac-medium | works |
| `--fil` | none exists | en model + indonesian phonemization so the vowels aren't mangled |
| `--jp` | ja_JP-hi_fi_captain-medium | needs openjtalk, skipped for now |
| `--kr` | ko_KR-kss-medium | works |
| `--cn` | zh_CN-huayan-medium | works |

models are gitignored, auto-downloaded. want a different voice, edit the table in `src/voice.rs`.

## pronunciation sounds wrong

`~/.config/coco-tts/pronunciation.tsv`, word TAB how it should sound:

```
jericho	jeh-rih-koh
```

iterate with `--print-phonemes` until it stops bugging you.

## how it works

espeak-ng makes IPA, that maps to the model's phoneme ids with a pad id after every symbol (that pad matters, skip it and "good morning" comes out "gudheng"), ort runs the vits model, rodio plays it, hound writes the wav if you asked.

playback pokes the default device with a bit of silence first. pipewire sometimes hands you a stream that never plays, so if the probe stalls it moves on to the next device instead of hanging.

needs rust, espeak-ng, and on linux `libasound2-dev`.

private repo, all rights reserved.
