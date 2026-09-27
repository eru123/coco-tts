use crate::voice::VoiceConfig;
use anyhow::{bail, Context, Result};
use std::process::Command;

/// A user pronunciation entry: a word as written, and the text it should be
/// spoken as instead (from the pronunciation dictionary file).
pub type Dictionary = Vec<(String, String)>;

/// Phoneme id sequences, one per sentence, each framed with BOS/EOS ids
/// (and a pause id when the sentence ended on punctuation), matching the
/// contract the Piper VITS models were exported with.
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
        let mut ids: Vec<i64> = Vec::new();
        extend_by_symbol(&mut ids, map, "^");
        for (i, word) in ipa.split_whitespace().enumerate() {
            if i > 0 {
                extend_by_symbol(&mut ids, map, " ");
            }
            for phone in word.split('|').filter(|p| !p.is_empty()) {
                extend_by_symbol(&mut ids, map, phone);
            }
        }
        if ids.len() > 1 {
            if let Some(ender) = ender {
                extend_ender(&mut ids, map, ender);
            }
            extend_by_symbol(&mut ids, map, "$");
            sentences.push(ids);
        }
    }
    if sentences.is_empty() {
        bail!("text produced no phonemes");
    }
    Ok(sentences)
}

/// The raw IPA espeak-ng produces for each sentence, for inspection.
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

/// Replace dictionary words with their spoken-as text. Matches whole words,
/// case-insensitively; anything that is not a letter, digit, apostrophe or
/// hyphen counts as a word boundary.
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

/// Map a phoneme to ids: the whole token first (multi-codepoint symbols),
/// then each codepoint on its own — symbols with no mapping are skipped,
/// which is how the upstream Piper pipeline treats them too.
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

fn extend_ender(ids: &mut Vec<i64>, map: &std::collections::HashMap<String, Vec<i64>>, ender: char) {
    let ascii_equivalent = match ender {
        '。' => Some('.'),
        '！' => Some('!'),
        '？' => Some('?'),
        '，' => Some(','),
        '；' => Some(';'),
        '：' => Some(':'),
        _ => None,
    };
    for candidate in [Some(ender), ascii_equivalent].into_iter().flatten() {
        if let Some(candidate_ids) = map.get(&candidate.to_string()) {
            ids.extend_from_slice(candidate_ids);
            return;
        }
    }
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
