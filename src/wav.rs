// WAV wrapping for pcm_s16le 16 kHz mono, and PCM level utilities.

const SAMPLE_RATE: u32 = 16_000;
const CHANNELS: u16 = 1;
const SAMPLE_WIDTH: u16 = 2;

/// Prepend a 44-byte WAV header (pcm_s16le, 16 kHz, mono) to raw PCM bytes.
pub fn wrap(pcm: &[u8]) -> Vec<u8> {
    let data_len = pcm.len() as u32;
    let byte_rate = SAMPLE_RATE * CHANNELS as u32 * SAMPLE_WIDTH as u32;
    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    out.extend_from_slice(&CHANNELS.to_le_bytes());
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&(CHANNELS * SAMPLE_WIDTH).to_le_bytes()); // block align
    out.extend_from_slice(&(SAMPLE_WIDTH * 8).to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(pcm);
    out
}

/// Peak absolute sample level; a trailing odd byte is dropped.
pub fn peak(pcm: &[u8]) -> i16 {
    pcm.as_chunks::<2>()
        .0
        .iter()
        .map(|c| i16::from_le_bytes([c[0], c[1]]) as i32)
        .map(i32::abs)
        .max()
        .map(|p| p.min(i16::MAX as i32) as i16)
        .unwrap_or(0)
}
