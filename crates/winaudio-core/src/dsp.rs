/// Converts multi-channel interleaved float32 audio to mono float32.
pub fn downmix_to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }

    let frame_count = samples.len() / channels;
    let mut mono = Vec::with_capacity(frame_count);
    let inv_ch = 1.0 / channels as f32;

    for frame in samples.chunks_exact(channels) {
        let sum: f32 = frame.iter().sum();
        mono.push(sum * inv_ch);
    }

    mono
}

/// Converts multi-channel interleaved float32 audio to stereo float32.
pub fn downmix_to_stereo(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels == 2 {
        return samples.to_vec();
    }
    if channels == 1 {
        let mut stereo = Vec::with_capacity(samples.len() * 2);
        for &s in samples {
            stereo.push(s);
            stereo.push(s);
        }
        return stereo;
    }

    let frame_count = samples.len() / channels;
    let mut stereo = Vec::with_capacity(frame_count * 2);

    for frame in samples.chunks_exact(channels) {
        // Simple surround/multi-channel folddown: L = ch0, R = ch1, C = ch2, etc.
        let left = frame[0];
        let right = frame[1];
        stereo.push(left);
        stereo.push(right);
    }

    stereo
}

/// Converts float32 audio [-1.0, 1.0] to signed 16-bit PCM [-32768, 32767].
pub fn f32_to_pcm16(samples: &[f32]) -> Vec<i16> {
    let mut pcm = Vec::with_capacity(samples.len());
    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let val = if clamped >= 0.0 {
            (clamped * 32767.0) as i16
        } else {
            (clamped * 32768.0) as i16
        };
        pcm.push(val);
    }
    pcm
}

/// Converts 16-bit PCM samples to raw little-endian bytes.
pub fn pcm16_to_bytes(samples: &[i16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    bytes
}

/// Fast real-time linear resampler that handles arbitrary chunk lengths without latency or buffer padding.
pub fn resample_linear(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || input.is_empty() {
        return input.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = ((input.len() as f64) / ratio).round() as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_idx = i as f64 * ratio;
        let idx0 = src_idx.floor() as usize;
        let frac = (src_idx - idx0 as f64) as f32;

        if idx0 + 1 < input.len() {
            let s0 = input[idx0];
            let s1 = input[idx0 + 1];
            output.push(s0 + frac * (s1 - s0));
        } else if idx0 < input.len() {
            output.push(input[idx0]);
        } else {
            output.push(0.0);
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downmix_to_mono() {
        let stereo = vec![1.0, -1.0, 0.5, 0.5];
        let mono = downmix_to_mono(&stereo, 2);
        assert_eq!(mono, vec![0.0, 0.5]);
    }

    #[test]
    fn test_f32_to_pcm16() {
        let floats = vec![0.0, 1.0, -1.0];
        let pcm = f32_to_pcm16(&floats);
        assert_eq!(pcm[0], 0);
        assert_eq!(pcm[1], 32767);
        assert_eq!(pcm[2], -32768);
    }

    #[test]
    fn test_resample_linear() {
        let input = vec![0.0, 0.5, 1.0, 0.5, 0.0];
        let resampled = resample_linear(&input, 48000, 24000);
        assert_eq!(resampled.len(), 3);
    }
}
