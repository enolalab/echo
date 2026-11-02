# Voice Dubbing (`--dub`) — Design

## Goal

Add an optional dubbing stage to the pipeline: after translation, synthesize each translated subtitle segment into speech via a pluggable TTS engine (local Piper binary or OpenAI-compatible cloud API), fit each audio clip into its subtitle time slot, assemble a single dubbed audio track, and mix it over the ducked original audio (default ~15%) in the output video. Works in both CLI and Echo Studio GUI, combined with `hardsub` and `softsub` modes.

## Non-goals

- Voice cloning or per-speaker voice assignment (single narrator voice for the whole video).
- SSML prosody/emotion tuning beyond engine defaults.
- Lip-sync or video retiming.
- Voice/music source separation (e.g. Demucs); the original track is simply ducked, so background music/ambience remains faintly audible.
- Edge-TTS engine in v1 (the `TtsEngine` trait allows adding it later without architecture changes).

## 1. Pipeline Integration & Data Flow

The dub stage runs after translation (segments carry translated text) and before output rendering. **Two call sites** share the `src/dub.rs` functions: `pipeline::run` for the CLI, and `export_subtitles` in `src-tauri/src/commands.rs` for the GUI. Note the GUI's `run_pipeline` (STT+translate) never renders video — it only returns `SubtitleSegment`s for editing; video/audio output happens later in `export_subtitles(ExportRequest)`. Dubbing at export time means the synthesized text reflects the user's subtitle edits.

```
Translated SubtitleSegments
  → DubStage:    TTS per segment → WAV clip
  → TimingFitter: fit clip into slot budget (overflow into gap, then atempo)
  → TrackBuilder: silence + clips concat → dubbed.wav (48 kHz mono s16)
  → Muxer:       ffmpeg amix(original@vol + dubbed) → output video
```

Mode interactions:

| `--mode` | `--dub` behavior |
|---|---|
| `hardsub` | Burn subtitles AND replace audio in a single ffmpeg pass (one video encode). |
| `softsub` | Mux subtitle track + replace audio stream (video stays `-c:v copy`). |
| `subs-only` | Emit `<stem>.dub.wav` next to `.srt`/`.ass`; video untouched. |

## 2. Components

### 2.1 `src/tts/` — TTS engines

```rust
#[async_trait]
pub trait TtsEngine: Send + Sync {
    /// Synthesize text → WAV bytes (engine-native sample rate).
    async fn synthesize(&self, text: &str) -> anyhow::Result<Vec<u8>>;
    fn name(&self) -> &'static str;
}
```

- `src/tts/mod.rs`: trait + `build_engine(cli) -> Box<dyn TtsEngine>`.
- `src/tts/piper.rs` (`PiperTts`): writes normalized segment text to the process's stdin and spawns `piper -m <voice>.onnx -c <voice>.onnx.json -f out.wav` per segment. Piper synthesizes a sentence in ~50–200 ms on CPU; model+espeak data reload per spawn adds ~0.3–1 s/segment — accepted in v1 for per-segment progress fidelity (piper batch stdin mode is a noted future optimization).
- `src/tts/api.rs` (`ApiTts`): `POST {api_base_url}/audio/speech` with `{model, voice, input, response_format: "wav"}` and `Authorization: Bearer <api-key>`. Reuses `--api-key` / `--api-base-url`; requires a provider that implements `/audio/speech` (errors early if no API key is configured). Returned bytes are normalized via ffmpeg regardless of codec, so providers that only return mp3 still work.

**Text normalization before `synthesize`:** flatten newlines to spaces and strip ASS override tags (`{...}`). Piper treats each stdin line as a separate utterance — without this, one segment could produce multiple wavs and break the clip↔segment mapping.

### 2.2 Piper binary & voice model management

Follows the existing ffmpeg/whisper/yt-dlp auto-download convention, **with one difference**: Piper is not a single static binary — its release is a bundle containing the `piper` executable plus shared libraries (`lib/`) and `espeak-ng-data/` which must stay co-located. Extraction therefore preserves the directory layout under `bin/piper/` (e.g. `bin/piper/piper`), unlike the single-file ffmpeg copy.

- Bundle lookup: system `PATH` (`which piper`) → `bin/piper/piper` in the app data dir (`~/.local/share/auto-subtitle/`) or CLI cache dir (`~/.cache/auto-subtitle/`).
- Download from GitHub releases (`rhasspy/piper`): `piper_linux_x86_64.tar.gz`, `piper_macos_{x64,aarch64}.tar.gz`, `piper_windows_amd64.zip`; extract preserving layout, `chmod +x` on Unix, verify with `piper --version`.
- Voice models are separate downloads stored under `tts/` in the same root. v1 ships a small built-in catalog:

| Voice ID | Language | HuggingFace path (`rhasspy/piper-voices`) | Quality |
|---|---|---|---|
| `vi_VN-vais1000-medium` | Vietnamese | `vi/vi_VN/vais1000/medium/` | medium — recommended default |
| `vi_VN-25hours_single-low` | Vietnamese | `vi/vi_VN/25hours_single/low/` | low — faster fallback |
| `en_US-lessac-medium` | English | `en/en_US/lessac/medium/` | medium |

Each voice requires both `<id>.onnx` and `<id>.onnx.json`. `--tts-voice` accepts a catalog ID or an absolute path to a local `.onnx` file (with sibling `.onnx.json`).

### 2.3 `src/dub.rs` — timing, track build, mux

- `plan_segments(&[SubtitleSegment]) -> Vec<DubPlan>` — computes per-segment slot budgets.
- `synthesize_all(engine, plans) -> Vec<DubClip>` — TTS each segment, skip failures with warning; abort if >50% of non-empty segments fail.
- `fit_clip` — normalize clip to 48 kHz mono s16 WAV via ffmpeg; applies `atempo` when required (same pass).
- `build_track(clips) -> PathBuf` — in-memory concat onto a silent timeline via `hound` (already a dependency), sample-accurate placement.
- `mux_dubbed(ffmpeg, video, subtitle_args, dubbed_wav, out, orig_volume)` — final ffmpeg pass.

## 3. Timing Algorithm

```text
slot(i)   = seg[i].end - seg[i].start
gap(i)    = seg[i+1].start - seg[i].end
gap(last) = video_end - seg.end             (video_end = `ffmpeg -i` stderr "Duration:" parse;
                                             if unavailable → gap = 0, degrades to atempo path)
budget(i) = slot(i) + min(gap(i) * 0.5, 2.0s)
MAX_TEMPO = 1.5

for each segment:
    if translated text is empty/whitespace → skip
    wav = engine.synthesize(text)
    if wav.duration <= budget → place at seg.start (may overflow into gap)
    else → tempo = clamp(wav.duration / budget, 1.0, 1.5)
           wav = ffmpeg atempo(tempo)
           place at seg.start (tiny residual overflow beyond budget is accepted)
```

Rationale: translation is often longer than the source; letting audio breathe ~50% into the following silence keeps speech natural, and `atempo` (≤1.5x) is the last resort so the dub almost never overlaps the next line. In the rare case a clamped clip still reaches into the next segment's start, `build_track` **sums overlapping samples** (both voices blend rather than truncate).

## 4. Track Assembly & Final Mux

1. Every clip is normalized to **48 kHz mono s16 WAV** in one ffmpeg pass (resample + atempo combined).
2. `hound` writes `dubbed.wav`: silence + clip samples placed at `seg.start` offsets. Track length only needs to reach the last clip's end — `amix duration=longest` handles tail mismatch.
3. Final mux:

```bash
ffmpeg -i <video> -i dubbed.wav \
  -filter_complex "[0:a]volume=<orig_vol>[a0];[1:a]apad[a1];[a0][a1]amix=inputs=2:duration=longest:normalize=0[aout]" \
  -map 0:v -map "[aout]" -c:v copy -c:a aac <out>
```

- `hardsub`: same pass, but the filter graph is a **single `-filter_complex`** containing both chains — `"[0:v]ass=<styled.ass>[v];[0:a]volume=...[a0];[1:a]apad[a1];[a0][a1]amix...[aout]"` with `-map "[v]" -map "[aout]"` (avoids `-vf`/`-filter_complex` mixing ambiguity).
- `softsub`: keep existing `-map`/`-c:s` subtitle args alongside `-map "[aout]"`.
- Video without an audio stream (e.g. `--sub` input on a silent file): skip `amix`, map `dubbed.wav` directly.

## 5. CLI Surface

| Flag | Default | Description |
|---|---|---|
| `--dub` | `false` | Enable voice dubbing of translated subtitles. |
| `--tts-engine <local\|api>` | `local` | `local` = Piper binary, `api` = OpenAI-compatible `/audio/speech`. |
| `--tts-voice <id-or-path>` | resolved from `--target-lang` (see below) / `alloy` (api) | Voice catalog ID, API voice name, or path to `.onnx`. |
| `--tts-model <name>` | `tts-1` | TTS model for `api` engine. |
| `--dub-orig-volume <0.0-1.0>` | `0.15` | Volume of the original audio under the dub. |

`--keep-temp` already preserves `dubbed.wav` and per-segment clips for debugging.

**Default voice resolution (local engine):** first catalog entry whose voice-ID prefix matches `--target-lang` (e.g. `vi` → `vi_VN-vais1000-medium`, `en` → `en_US-lessac-medium`). If no catalog voice matches the target language → error telling the user to pass `--tts-voice` (catalog ID or `.onnx` path) or `--tts-engine api`. This avoids e.g. a Vietnamese voice reading English text when `--target-lang en`.

## 6. GUI (Echo Studio)

### 6.1 Settings (`SettingsModal.tsx`)

New **TTS / Lồng tiếng** section:

- Engine select: `Local (Piper)` / `Cloud API`.
- Voice input with datalist of catalog voices (local) or free text (API voice name).
- API TTS model name input (visible when engine = api).
- Original-audio volume slider `0–50%` (maps to `0.0–0.5`).
- Piper status indicator + "Tải Piper" button (same pattern as `yt-dlp` status in Settings).

### 6.2 Job creation & progress

- `ExportModal` gains a **"Lồng tiếng"** toggle next to the bilingual option; `ExportRequest` gains an optional `dub: Option<DubOptions>` field (`#[serde(default)]`). Defaults come from `GeneralSettings`.
- During `export_subtitles`, when `dub.enabled`, the dub stage runs before the final ffmpeg pass and emits a new `export-progress` event (`{ index, total }`) → `ExportModal` shows inline `"Đang lồng tiếng 12/84"`; a `pipeline-log` line is also emitted (existing channel, zero extra plumbing). `ExportRequest` gains an optional `job_id`; when present the command registers a cancel sender and checks `cancel_rx` between segments, so the existing `cancel_pipeline` command can abort a long dub.
- Output player is unchanged: it already plays the rendered output file, which now carries the dubbed audio.

### 6.3 Tauri commands

- `check_piper_status() -> PiperStatus` — `{installed, path, version}` like `check_yt_dlp_status`.
- `ensure_piper() -> PiperStatus` — download binary bundle + default voice if missing.
- `export_subtitles(app, request)` — gains `AppHandle` (for progress/log events) and handles the optional `dub` field per §1–§4.

## 7. Data Models

```rust
// src/tts/mod.rs (shared by CLI & Tauri)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DubOptions {
    pub enabled: bool,
    pub engine: TtsEngineKind,      // Local | Api
    pub voice: Option<String>,      // catalog id / api voice / .onnx path
    pub model: Option<String>,      // api tts model
    pub orig_volume: f32,           // 0.0..=1.0, default 0.15
}

// src-tauri/src/types.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiperStatus {
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
}

// new event emitted during GUI export dubbing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportProgress {
    pub job_id: Option<String>,
    pub stage: String,        // "dubbing"
    pub index: usize,
    pub total: usize,
}

// ExportRequest additions (all optional, #[serde(default)])
pub dub: Option<DubOptions>;
pub job_id: Option<String>;
```

## 8. Error Handling & Edge Cases

| Scenario | Handling |
|---|---|
| Piper binary missing | Auto-download on first `--dub` run (CLI prints progress; GUI prompts via `ensure_piper`). Download failure → error with manual-install hint, same as ffmpeg flow. |
| Voice model missing for chosen voice | Download from catalog; if `--tts-voice` is a path, require sibling `.onnx.json` else clear error. |
| TTS fails on a segment | Log warn, skip that segment (silence in its slot), continue. |
| >50% of segments fail | Abort with error — a mostly-silent dub is worse than none. |
| `api` engine HTTP error | Fail that segment; provider auth/quota errors surface with response body in the warn log. |
| Empty/whitespace translated text | Skip synthesis for that segment. |
| Segment with zero/negative slot | Still synthesize; budget degenerates to `min(gap*0.5, 2.0s)` + atempo path. |
| Input video has no audio stream | Mux dubbed track directly without `amix`. |
| `--skip-translate` + `--dub` | Dub `original_text` as-is — flag combination is allowed. |

## 9. Testing Strategy

1. **Unit tests (`src/dub.rs`)**:
   - `plan_segments`: slot/gap/budget math, last-segment handling, zero-gap back-to-back segments.
   - Fit decision table: fits-in-slot, overflows-into-gap, needs-atempo, tempo-clamped-at-1.5.
   - `build_track`: sample-accurate placement (write two clips, assert non-zero samples at expected offsets), silence elsewhere.
2. **Engine tests**:
   - `ApiTts` via `wiremock` (existing dev-dep): happy path WAV bytes, 4xx/5xx → segment failure path.
   - `PiperTts` arg construction; voice-path validation (missing `.onnx.json` error).
   - ffmpeg command builder tests: filter graph string contains `volume`, `atempo`, `amix`; `--sub`-input no-audio variant maps `dubbed.wav` directly.
3. **Integration smoke**: tiny fixture video + feature-gated `MockTtsEngine` returning a generated sine WAV → assert output file has an AAC audio stream and expected duration.
4. **Manual GUI verification** (repo has no UI test harness): toggle dub on a job, watch `dubbing` progress stage, confirm player output audio.

## 10. Acceptance Criteria

- `auto-subtitle -i video.mp4 --dub` produces a video whose audio track contains intelligible Vietnamese speech aligned with subtitle timings, original audio faintly audible underneath.
- `--dub --mode softsub` and `--dub --mode subs-only` behave per the mode table (muxed track / emitted `.dub.wav`).
- A segment whose TTS audio exceeds its slot is sped up ≤1.5x and never overlaps the next segment's start beyond the summed-overflow rule.
- First run without Piper downloads binary bundle + default voice automatically; `--dub --tts-engine api` without API key fails fast with a clear message.
- GUI: toggling "Lồng tiếng" in Export modal produces the same dubbed output as the CLI, shows `export-progress`/`pipeline-log` status, and cancel during dubbing aborts cleanly.
