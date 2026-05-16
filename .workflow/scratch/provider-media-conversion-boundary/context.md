# M7-T2p Provider Parity: Audio Media Conversion Boundary

## Reference

- AstrBot keeps Tencent/SILK/AMR conversion in `tencent_record_helper.py`.
- AstrBot OpenAI Whisper STT and Xinference STT both detect HTTP audio, Tencent multimedia URLs, SILK headers, and AMR headers/extensions before sending audio to provider APIs.
- Concrete conversion depends on Python-only helpers (`pysilk`, `pilk`, `pyffmpeg`/`ffmpeg`) and temporary files.

## Rust Decision

- Add `AudioInputLoader` in the provider crate to own local/HTTP audio loading for STT adapters.
- Add `AudioFormat`, `AudioConversionRequest`, `AudioMediaConverter`, `UnsupportedAudioMediaConverter`, and `detect_audio_conversion_requirement` as the first shared media conversion boundary.
- Keep the default converter explicitly unsupported, returning a clear provider error when SILK/AMR/Tencent conversion is required.
- Refactor OpenAI Whisper STT and Xinference STT to use the shared loader, preserving auth isolation for HTTP audio downloads.
- Do not bind ffmpeg, Tencent SILK helpers, or platform-specific conversion into concrete provider adapters. A later runtime/media adapter can provide an `AudioMediaConverter` implementation.

## Verification

- PASS: `cargo test -p astrbot-provider`
- PASS: `cargo fmt --all --check`
- PASS: `cargo test --workspace`
- PASS: `cargo clippy --workspace -- -D warnings`
- PASS: `cargo run -p astrbot-cli`
- PASS: `.workflow` JSON parse check
