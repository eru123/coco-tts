use crate::voice::VoiceConfig;
use anyhow::{bail, Context, Result};
use std::process::Command;

/// word -> say it like this, from the pronunciation dictionary file
pub type Dictionary = Vec<(String, String)>;

/// ids per sentence, BOS/EOS framed. what the piper models expect.
pub fn phoneme_ids(
    config: &VoiceConfig,
    espeak_voice: &str,
    dictionary: &Dictionary,
    text: &str,
) -> Result<Vec<Vec<i64>>> {
    if config.phoneme_type != "espeak" {
        bail!(
            "voice uses '{}' phonemization, which is not supported yet",
            config.phoneme_type
        );
    }
    let text = apply_dictionary(text, dictionary);
    let map = &config.phoneme_id_map;
    let mut sentences = Vec::new();
    for (body, ender) in split_sentences(&text) {
        let ipa = espeak_ipa(espeak_voice, &body)?;
        let mut symbols: Vec<char> = Vec::new();
        for (i, word) in ipa.split_whitespace().enumerate() {
            if i > 0 {
                symbols.push(' ');
            }
            for phone in word.split('|').filter(|p| !p.is_empty()) {
                // piper phonemes are single codepoints, diphthongs included
                symbols.extend(phone.chars());
            }
        }
        if symbols.is_empty() {
            continue;
        }
        if let Some(ender) = ender.and_then(|e| resolve_ender(map, e)) {
            symbols.push(ender);
        }

        // piper pads every symbol, bos included, eos not. skip the pads and
        // words mush together (good morning -> gudheng)
        let mut ids: Vec<i64> = Vec::new();
        extend_by_symbol(&mut ids, map, "^");
        extend_by_symbol(&mut ids, map, "_");
        for symbol in &symbols {
            extend_by_symbol(&mut ids, map, &symbol.to_string());
            extend_by_symbol(&mut ids, map, "_");
        }
        extend_by_symbol(&mut ids, map, "$");
        sentences.push(ids);
    }
    if sentences.is_empty() {
        bail!("text produced no phonemes");
    }
    Ok(sentences)
}

/// raw IPA per sentence, for --print-phonemes
pub fn ipa_sentences(espeak_voice: &str, dictionary: &Dictionary, text: &str) -> Result<Vec<String>> {
    let text = apply_dictionary(text, dictionary);
    let mut out = Vec::new();
    for (body, ender) in split_sentences(&text) {
        let mut ipa = espeak_ipa(espeak_voice, &body)?;
        if let Some(ender) = ender {
            ipa.push(ender);
        }
        out.push(ipa);
    }
    Ok(out)
}

/// swap dictionary words for their respelling. whole words, case-insensitive
fn apply_dictionary(text: &str, dictionary: &Dictionary) -> String {
    if dictionary.is_empty() {
        return text.to_string();
    }
    let is_word_char = |ch: char| ch.is_alphanumeric() || ch == '\'' || ch == '-';
    let mut out = String::with_capacity(text.len());
    let mut pending = String::new();
    for ch in text.chars() {
        if is_word_char(ch) {
            pending.push(ch);
        } else {
            if !pending.is_empty() {
                out.push_str(&lookup_word(&pending, dictionary));
                pending.clear();
            }
            out.push(ch);
        }
    }
    out.push_str(&lookup_word(&pending, dictionary));
    out
}

fn lookup_word(word: &str, dictionary: &Dictionary) -> String {
    let lower = word.to_lowercase();
    for (entry, replacement) in dictionary {
        if *entry == lower {
            return replacement.clone();
        }
    }
    word.to_string()
}

/// whole token first, then per codepoint. unmapped symbols get skipped,
/// same as piper
fn extend_by_symbol(ids: &mut Vec<i64>, map: &std::collections::HashMap<String, Vec<i64>>, symbol: &str) {
    if let Some(symbol_ids) = map.get(symbol) {
        ids.extend_from_slice(symbol_ids);
        return;
    }
    for ch in symbol.chars() {
        if let Some(ch_ids) = map.get(&ch.to_string()) {
            ids.extend_from_slice(ch_ids);
        }
    }
}

/// pause symbol for a sentence ender, CJK falls back to ascii
fn resolve_ender(
    map: &std::collections::HashMap<String, Vec<i64>>,
    ender: char,
) -> Option<char> {
    let ascii_equivalent = match ender {
        '。' => Some('.'),
        '！' => Some('!'),
        '？' => Some('?'),
        '，' => Some(','),
        '；' => Some(';'),
        '：' => Some(':'),
        _ => None,
    };
    [Some(ender), ascii_equivalent]
        .into_iter()
        .flatten()
        .find(|candidate| map.contains_key(&candidate.to_string()))
}

fn split_sentences(text: &str) -> Vec<(String, Option<char>)> {
    let is_sentence_ender =
        |ch: char| matches!(ch, '.' | '!' | '?' | ';' | ':' | ',' | '。' | '！' | '？' | '；' | '：' | '，');

    let mut parts = Vec::new();
    let mut body = String::new();
    for ch in text.chars() {
        if is_sentence_ender(ch) {
            parts.push((std::mem::take(&mut body), Some(ch)));
        } else {
            body.push(ch);
        }
    }
    parts.push((body, None));
    parts
        .into_iter()
        .filter(|(body, _)| !body.trim().is_empty())
        .map(|(body, ender)| (body.trim().to_string(), ender))
        .collect()
}

fn espeak_ipa(voice: &str, text: &str) -> Result<String> {
    let output = Command::new("espeak-ng")
        .args(["-q", "--ipa", "--sep=|", "-v", voice, "--", text])
        .output()
        .context("failed to run espeak-ng; install it (apt install espeak-ng) and retry")?;
    if !output.status.success() {
        bail!(
            "espeak-ng failed for voice '{voice}': {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
