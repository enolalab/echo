# Spec Review: Voice Dubbing (`--dub`)

**Mode:** Focused
**Scope:** full spec (single phase)
**Spec:** `docs/superpowers/specs/2026-11-02-voice-dubbing-design.md`
**Base:** commit `0745302`
**Reviewers:** Technical (performed directly — no subagents dispatched)

## Verdict: Needs Changes → Approved with Follow-ups (after fixes)

Problem, goal, and non-goals are clear and the overall architecture (pluggable `TtsEngine` + `src/dub.rs` timing/track/mux) fits existing conventions well. Five material gaps found by verifying spec claims against the codebase and external tool facts — all fixed in the spec in one editing pass (see diff after `0745302`). No Critical findings; core approach is sound.

## Blocking Findings (all resolved in-place)

1. **F1 Piper artifact is a directory bundle, not a single binary** — Important — Blocking: yes
   - Evidence: §2.2 says "spawns `piper` binary ... into `bin/piper`". Piper releases are tarballs/zips containing `piper` + shared libs + `espeak-ng-data/` that must stay co-located; unlike the single-file ffmpeg static binary the current `find_or_download_ffmpeg` flow renames into `bin/`.
   - Impact: plan/implementation would design a wrong download+layout step; binary fails to run without bundled data/libs.
   - Minimal fix: spec now states extract preserving layout under `bin/piper/`, invoke `<bundle>/piper`; voice `.onnx`+`.onnx.json` downloaded separately into `tts/`.

2. **F2 `video_end` for last-segment gap has no source** — Important — Blocking: yes
   - Evidence: §3 defines `gap(last) = video_end - seg.end` but nothing in the spec or codebase provides `video_end` (no ffprobe is downloaded).
   - Impact: plan authors must invent the duration source; wrong choice breaks the budget formula.
   - Minimal fix: spec now defines duration = parse `ffmpeg -i` stderr `Duration:`; fallback `gap(last) = 0` (degrades safely to the atempo path).

3. **F3 Dub stage must hook two pipelines, not one** — Important — Blocking: yes
   - Evidence: spec describes one pipeline hook, but the GUI does not call `pipeline::run` — `src-tauri/src/commands.rs` reimplements the pipeline inline with its own progress/cancel handling (`emit_progress`, `cancel_rx`).
   - Impact: high chance the plan wires only the CLI path; GUI toggle would silently do nothing.
   - Minimal fix: §1/§6 now state the dub stage is shared functions in `src/dub.rs` invoked from both `pipeline::run` and the Tauri inline pipeline, with `dubbing` progress + cancel checks in the latter.

4. **F4 Voice default must follow `--target-lang`** — Important — Blocking: yes
   - Evidence: §5 defaults `--tts-voice` to `vi_VN-vais1000-medium` unconditionally; a `vi_VN` voice reading non-Vietnamese translated text produces garbage for `--target-lang en` etc.
   - Impact: wrong output for any non-`vi` target; material behavior decision.
   - Minimal fix: default voice = first catalog entry whose ID prefix matches `target_lang`; no match → error advising `--tts-voice` or `--tts-engine api`.

5. **F5 Newlines/ASS tags in segment text break per-segment wav mapping** — Important — Blocking: yes
   - Evidence: §2.1 sends segment text to piper stdin; piper treats each stdin line as a separate utterance, so a segment containing `\n` yields multiple wavs and breaks the clip↔segment mapping.
   - Impact: silent misalignment of dubbed audio for multi-line segments.
   - Minimal fix: spec now requires text normalization (flatten newlines, strip ASS override tags `{...}`) before `synthesize`.

## Follow-ups (non-blocking, recorded)

- **M1:** Piper reloads model + espeak data per spawn (~0.3–1 s/segment); accepted in v1 for per-segment progress fidelity. Batch stdin mode (`--json-input`/`--output-dir`) is a noted future optimization.
- **M2:** hardsub + dub uses a single `-filter_complex` containing both the `ass` burn chain and audio chains (avoids `-vf`/`-filter_complex` mixing ambiguity).
- **M3:** `api` engine requires a provider implementing `POST /audio/speech` and an `--api-key`; synthesized bytes are normalized via ffmpeg regardless of returned codec, so mp3-capable providers still work.
- **M4:** Added explicit measurable acceptance criteria.
- **M5:** Clarified `--skip-translate --dub` synthesizes `original_text`.

## Coverage and Resolution

- Engine trait & factory → §2.1 → covered
- Piper/voice download & layout → §2.2 → resolved (F1)
- Timing budget/atempo → §3 → resolved (F2, F5)
- Track build & mux filters → §4 → covered (M2)
- CLI flags & defaults → §5 → resolved (F4)
- GUI integration & progress → §6 → resolved (F3)
- Error/edge matrix → §8 → covered
- Acceptance criteria → §10 → resolved (M4)

Strengths kept: single-pass burn+mux design, `hound` in-memory track build (dep already present), honest non-goals.
