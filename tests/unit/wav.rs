// Story: 001 — WAV wrapping and PCM level utilities.

use vibe_audio_bridge::wav;

#[test]
fn test_wav_header_fields() {
    let pcm = vec![0xABu8; 100];
    let out = wav::wrap(&pcm);

    assert_eq!(&out[0..4], b"RIFF");
    assert_eq!(u32::from_le_bytes(out[4..8].try_into().unwrap()), 136); // 36 + 100
    assert_eq!(&out[8..12], b"WAVE");
    assert_eq!(&out[12..16], b"fmt ");
    assert_eq!(u32::from_le_bytes(out[16..20].try_into().unwrap()), 16); // fmt chunk size
    assert_eq!(u16::from_le_bytes(out[20..22].try_into().unwrap()), 1); // PCM
    assert_eq!(u16::from_le_bytes(out[22..24].try_into().unwrap()), 1); // mono
    assert_eq!(u32::from_le_bytes(out[24..28].try_into().unwrap()), 16000); // sample rate
    assert_eq!(u32::from_le_bytes(out[28..32].try_into().unwrap()), 32000); // byte rate
    assert_eq!(u16::from_le_bytes(out[32..34].try_into().unwrap()), 2); // block align
    assert_eq!(u16::from_le_bytes(out[34..36].try_into().unwrap()), 16); // bits per sample
    assert_eq!(&out[36..40], b"data");
    assert_eq!(u32::from_le_bytes(out[40..44].try_into().unwrap()), 100); // data size
    assert_eq!(&out[44..], &pcm[..]);
    assert_eq!(out.len(), 144);
}

#[test]
fn test_peak_of_pcm_samples() {
    let mut pcm = Vec::new();
    for s in [0i16, 100, -200, 3] {
        pcm.extend_from_slice(&s.to_le_bytes());
    }
    assert_eq!(wav::peak(&pcm), 200);
}

#[test]
fn test_peak_of_empty_and_odd_length_pcm() {
    assert_eq!(wav::peak(&[]), 0);
    // Trailing byte is dropped, like the Python bridge did.
    let mut pcm = Vec::new();
    pcm.extend_from_slice(&1000i16.to_le_bytes());
    pcm.push(0xFF);
    assert_eq!(wav::peak(&pcm), 1000);
}
