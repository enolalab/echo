# Plan Review: Auto-Subtitle Rust CLI

**Mode:** Full (user requested review of all aspects)
**Scope:** Entire plan (Tasks 1–8) + global constraints
**Plan:** `docs/superpowers/plans/2026-10-31-auto-subtitle-rust-cli.md`
**Spec:** `docs/superpowers/specs/2026-10-31-auto-subtitle-rust-cli-design.md`
**Base:** `bcc12f9`
**Reviewers:** Technical, Product/Spec-coverage, Process (build/toolchain), Data-safety, Performance — performed directly (no subagents)

## Verdict: Needs Changes → fixed in revised plan

The original plan had good module boundaries and TDD structure, but it would not compile as written, its default path (local Whisper) was a stub that always produced empty subtitles, and the translation step silently dropped every batch after the first. Several spec requirements (STT engine flags, `.ass` input parsing, Ollama health check, CUDA→CPU fallback, timecode sanitization, E2E test with mocks) had no task. All findings below were fixed in the revised plan.

Strengths kept: trait-based `Transcriber`/`Translator` boundaries; FFmpeg command builders separated from execution (testable); batch translation for context.

## Blocking Findings

1. **C1 Default local STT path not implemented** — Critical — Blocking: yes
   - Evidence: Task 7 `LocalWhisperTranscriber::transcribe` returns `Ok(Vec::new())`; `whisper-rs` never added to `Cargo.toml`; Task 8 `main.rs` sets `let segs = Vec::new()` and never calls any `Transcriber` or `ensure_whisper_model`. Flags `--stt-engine`, `--whisper-model` absent.
   - Impact: The headline feature (video → subtitles) produces an empty video subtitle track.
   - Fix: Real `whisper-rs` implementation (pinned `0.14`, `hound` WAV reader, `spawn_blocking`, GPU→CPU retry), `cuda` cargo feature, wire `--stt-engine`/`--whisper-model` in pipeline; fail with "no speech detected" when empty.

2. **C2 Plan does not compile** — Critical — Blocking: yes
   - Evidence: Task 4 uses `reqwest::blocking` but feature `blocking` not enabled; Task 7 uses `reqwest::multipart` without `multipart` feature; blocking reqwest called inside `#[tokio::main]` panics at runtime; Task 8 test imports `clap::Parser` but `Cli` lives in the binary crate (untestable from `tests/`).
   - Impact: `cargo test` fails at Task 4 / Task 7; runtime panic on first download.
   - Fix: Async downloader with `bytes_stream()`; enable `multipart`; move `Cli` into `src/cli.rs` (lib) and test via `Cli::try_parse_from`.

3. **C3 Translation drops all batches after the first** — Critical — Blocking: yes
   - Evidence: Task 6 prompt numbers lines with global `seg.id` (21..40 in batch 2) while `parse_translated_lines` only accepts `1..=count`; Task 8 also swallows errors with `if let Ok(..)`.
   - Impact: Only the first 20 lines are translated; failures are invisible.
   - Fix: Number lines locally `1..=n` per batch; `parse_translated_lines` returns `Err` when any line is missing; pipeline retries each batch up to 3 times then fails loudly with the batch range.

4. **C4 Output can overwrite user's input files** — Critical (data loss) — Blocking: yes
   - Evidence: Task 8 writes `<stem>.srt` into CWD. With `-i video.mp4 --sub video.srt`, the user's original subtitle is overwritten. No guard against `--output` == `--input`.
   - Impact: Irrecoverable loss of user input.
   - Fix: Intermediate files go to a `tempfile` work dir; exported subs named `<out_stem>.<target_lang>.srt/.ass` next to the output; canonical-path guard rejecting any output that equals an input.

5. **I1 Wrong expected value in Task 1 test + FFmpeg filter path injection** — Important — Blocking: yes
   - Evidence: `format_timestamp_srt(1234)` asserted as `"00:00:01,123"` (correct: `"00:00:01,234"`) → test can never pass. Task 5 builds `-vf ass=<raw path>`; paths with `:`, `'`, `,`, `\` or spaces break the filtergraph; `to_str().unwrap()` panics on non-UTF-8.
   - Impact: Task 1 stuck red; hardsub fails on ordinary paths (e.g. `~/Videos/My Clip: Part 1.mp4`).
   - Fix: Correct the assertion; run hardsub with `current_dir(workdir)` and a fixed safe file name `subtitle.ass`, absolute input/output paths.

## Non-blocking findings (also fixed)

- **I2 Toolchain prerequisites missing:** `whisper-rs` needs `cmake` + `libclang` (bindgen); CUDA build needs `nvcc`. None are installed on this machine (`cmake`, `clang`, `nvcc`, `ollama` not found). → Added Task 0 preflight + docs; `whisper-rs` deferred to Task 7 so Tasks 1–6 build fast.
- **I3 Partial model download treated as valid:** download wrote directly to final path. → `.part` + atomic rename, `error_for_status()`, model-name allow-list (prevents URL/path injection).
- **I4 Softsub container handling:** always `mov_text` without `-map` → fails for `.mkv`. → codec chosen by output extension (`mp4/mov/m4v → mov_text`, `mkv → srt`), explicit `-map 0:v -map 0:a? -map 1:0`.
- **I5 API STT 25 MB upload limit:** 16 kHz WAV ≈ 1.9 MB/min → fails after ~13 min. → API mode extracts 32 kbps mono MP3 (~0.24 MB/min) and checks size before upload.
- **I6 Spec coverage gaps:** `.ass` input parsing, Ollama health check + model-exists check, timecode sanitization, ASS override-tag escaping, `--api-model`, E2E test with mock transcriber/translator, `indicatif` progress. → each mapped to a task below.
- **M1** FFmpeg errors now include stderr tail (`-hide_banner -loglevel error`).
- **M2** VTT cue settings (`align:start`), BOM, `NOTE` blocks handled.

## Follow-ups (not in this plan)

- `--font` option for ASS (current default Arial, relies on fontconfig fallback for Vietnamese glyphs).
- Passing Whisper-detected language to translator prompt instead of `auto`.
- Chunked upload for API STT on videos > ~100 min.

## Coverage and Resolution

- Spec §1 Hybrid local/cloud STT → Task 7 + Task 8 → covered
- Spec §1 Existing subtitle input (`.srt/.vtt/.ass`) + `--skip-translate` → Task 2 + Task 8 → covered
- Spec §3 Data models & traits → Tasks 1, 6, 7 → covered
- Spec §4 CLI flags → Task 8 (`src/cli.rs`) → covered
- Spec §5 Zero-config FFmpeg/model download → Task 4 → covered
- Spec §6.1 Ollama health check → Task 6 → covered
- Spec §6.2 CUDA → CPU fallback → Task 7 → covered
- Spec §6.3 Timecode sanitization / ASS escaping → Tasks 1, 3 → covered
- Spec §7 Unit + E2E tests with mocks → every task + Task 8 → covered
- C1–C4, I1–I6, M1–M2 → revised plan → resolved
