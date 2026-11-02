# Plan Review: Voice Dubbing

**Mode:** Focused
**Scope:** Full plan (10 tasks) — contracts verified against `pipeline.rs`, `media.rs`, `downloader.rs`, `commands.rs`, `types.rs`, `Cargo.toml`
**Plan:** `docs/superpowers/plans/2026-11-02-voice-dubbing.md`
**Spec:** `docs/superpowers/specs/2026-11-02-voice-dubbing-design.md`
**Base:** commit `f6ad863`
**Reviewers:** Technical (direct, no subagents)

## Verdict: Needs Changes

Architecture, task ordering, and interfaces are sound — all referenced identifiers verified real (`Deps`, `run_ffmpeg(cmd, what)`, `download_to(&self.client,..)`, `SubtitleSegment::new`, `state.cancel_senders`, `get_app_config()`, `settings.lock()`/`stt_key()`, `video_in`/`out_path`/`temp_ass`/`temp_srt` in `export_subtitles`). All needed crates already in Cargo.toml (which, tempfile, regex, hound, wiremock, async-trait). Three code-level issues would break compile/runtime during execution.

## Blocking Findings

1. **F1 `is_cancelled` bound `Fn()` won't compile** — Important — Blocking: yes
   - Evidence: Task 6 signature `C: Fn() -> bool + Send`; Task 9 GUI closure `move || cancel_rx.try_recv().is_ok()`. `tokio::sync::broadcast::Receiver::try_recv(&mut self)` requires `&mut` — a `Fn` closure cannot do this.
   - Impact: Task 9 code as written fails to compile.
   - Minimal fix: bound `C: FnMut() -> bool + Send`, param `mut is_cancelled: C`; GUI side binds `let mut rx = cancel_rx` then `move || rx.try_recv().is_ok()`.

2. **F2 Piper `releases/latest` URL is brittle** — Important — Blocking: yes
   - Evidence: Task 2 `https://github.com/rhasspy/piper/releases/latest/download/piper_linux_x86_64.tar.gz`. The `rhasspy/piper` repo moved to `OHF-Voice/piper1-gpl`; its latest releases are the Python package, not guaranteed to carry the C++ tarball assets.
   - Impact: first-run auto-download can 404.
   - Minimal fix: pin tag `2023.11.14-2` (`releases/download/2023.11.14-2/...`); add macOS `x64`/`aarch64` tar.gz assets (same tar extraction path); Windows stays manual-PATH (zip needs new dep — documented limitation, consistent with ffmpeg's Linux-only policy).

3. **F3 Piper spawn missing `LD_LIBRARY_PATH`** — Important — Blocking: yes
   - Evidence: Task 3 `PiperTts::synthesize` sets `current_dir` but no env. The release bundle ships `.so` files (libonnxruntime, libpiper_phonemize, libespeak-ng) beside/under the binary; without lib path the binary may fail to start even when found.
   - Impact: local TTS runtime failure after a successful "found" check.
   - Minimal fix: in `synthesize`, `.env("LD_LIBRARY_PATH", "<exe_dir>/lib:<exe_dir>:$LD_LIBRARY_PATH")` (covers both flat and `lib/` layouts; harmless if rpath already set).

## Follow-ups (fixed in same pass)

- **M1** `cancel_senders` entry for export job never removed → remove after dub completes/errors.
- **M2** Vacuous assert `track.iter().take(...).all(|&s| s != 0 || true)` in Task 6 test → drop.
- **M3** `probe_media` runs twice (once inside `render_dub_track`, once in mux dispatch for `has_audio`) → `render_dub_track` returns `(PathBuf /*wav*/, bool /*has_audio*/)`; Tasks 8/9 consume the flag.
- **M4** `ensure_piper` Tauri command should take `state: State<AppState>` and resolve default voice from `settings().general.target_lang` (matches spec default-voice rule) instead of hardcoded `"vi"`.
- **M5** `main.rs` needs `use std::path::PathBuf;` for `resolve_local_voice` signature.

## Coverage and Resolution

- TTS trait + engines → Tasks 1/3/4 → covered
- Piper bundle + voice download → Task 2 → covered (with F2/F3 fixes)
- Timing algorithm (slot/gap/budget/atempo) → Task 5 → covered
- Track assembly + mux commands → Tasks 6/7 → covered (M2/M3)
- CLI flags + pipeline stage → Task 8 → covered
- GUI export-time dub + progress + cancel → Task 9 → covered (with F1/M1/M4 fixes)
- UI toggles/settings → Task 10 → covered
- Error matrix (>50% abort, empty text, no-audio, skip-translate→original_text, cancel) → covered in Tasks 5/6/8/9
