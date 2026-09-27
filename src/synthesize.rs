use crate::voice::LoadedVoice;
use anyhow::{Context, Result};
use ort::session::Session;
use ort::value::Tensor;

pub struct Audio {
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

/// Run each sentence's phoneme ids through the VITS model and stitch the
/// results together with a short silence between sentences.
pub fn run(voice: &LoadedVoice, sentences: &[Vec<i64>]) -> Result<Audio> {
    let mut session = Session::builder()
        .and_then(|mut b| b.commit_from_file(&voice.model_path))
        .with_context(|| format!("failed to load onnx model {}", voice.model_path.display()))?;
    let has_sid = session.inputs().iter().any(|input| input.name() == "sid");

    let sample_rate = voice.config.audio.sample_rate;
    let inter_sentence_silence = sample_rate as usize / 5;
    let mut samples: Vec<f32> = Vec::new();

    for (index, ids) in sentences.iter().enumerate() {
        if index > 0 {
            samples.extend(std::iter::repeat(0.0).take(inter_sentence_silence));
        }

        let len = ids.len();
        let input = Tensor::<i64>::from_array(([1usize, len], ids.clone()))
            .context("failed to build input tensor")?;
        let input_lengths = Tensor::<i64>::from_array(([1usize], vec![len as i64]))
            .context("failed to build input_lengths tensor")?;
        let scales = Tensor::<f32>::from_array((
            [3usize],
            vec![
                voice.config.inference.noise_scale(),
                voice.config.inference.length_scale(),
                voice.config.inference.noise_w(),
            ],
        ))
        .context("failed to build scales tensor")?;

        let outputs = if has_sid {
            let sid = Tensor::<i64>::from_array(([1usize], vec![0]))
                .context("failed to build sid tensor")?;
            session.run(ort::inputs![
                "input" => input,
                "input_lengths" => input_lengths,
                "scales" => scales,
                "sid" => sid
            ])
        } else {
            session.run(ort::inputs![
                "input" => input,
                "input_lengths" => input_lengths,
                "scales" => scales
            ])
        }
        .context("onnx inference failed")?;

        let output = outputs
            .get("output")
            .context("model produced no output tensor")?;
        let (_, data) = output
            .try_extract_tensor::<f32>()
            .context("failed to extract audio from the output tensor")?;
        let valid_len = outputs
            .get("output_lengths")
            .and_then(|value| value.try_extract_tensor::<i64>().ok())
            .map(|(_, lengths)| lengths[0].max(0) as usize)
            .unwrap_or(data.len())
            .min(data.len());
        samples.extend_from_slice(&data[..valid_len]);
    }

    Ok(Audio { sample_rate, samples })
}
