use std::path::{Path, PathBuf};

use anyhow::Context;
use indicatif::{ProgressBar, ProgressStyle};

use crate::cli::{Cli, OutputMode, SttEngine};
use crate::stt::Transcriber;
use crate::subtitle::model::SubtitleSegment;
use crate::translate::Translator;

pub const BATCH_SIZE: usize = 20;
pub const MAX_ATTEMPTS: usize = 3;

#[derive(Debug, Clone)]
pub struct OutputPaths {
    pub video: Option<PathBuf>,
    pub srt: PathBuf,
    pub ass: PathBuf,
}

fn canonical_path(p: &Path) -> PathBuf {
    if let Ok(c) = std::fs::canonicalize(p) {
        return c;
    }
    let parent = p.parent().unwrap_or_else(|| Path::new(""));
    let parent = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    if let Ok(pc) = std::fs::canonicalize(parent) {
        if let Some(file_name) = p.file_name() {
            return pc.join(file_name);
        }
    }
    std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Defaults: video = <input_dir>/<stem>_subtitled.<ext> (ext = input ext for softsub, "mp4" for hardsub; None for subs-only).
/// out_stem = stem of `-o` if given, else "<input_stem>_subtitled"; output_dir = parent of `-o` if given, else input dir.
/// srt/ass  = <output_dir>/<out_stem>.<target_lang>.srt|.ass  (exported for subs-only, or with --keep-temp).
/// Err if any output path canonically equals cli.input or cli.subtitle.
pub fn resolve_output_paths(cli: &Cli) -> anyhow::Result<OutputPaths> {
    let input_stem = cli
        .input
        .file_stem()
        .and_then(|s| s.to_str())
        .context("input file path has no valid file stem")?;

    let input_dir = cli.input.parent().unwrap_or_else(|| Path::new(""));

    let (output_dir, out_stem) = if let Some(ref out) = cli.output {
        if out.is_dir() {
            (out.as_path(), format!("{input_stem}_subtitled"))
        } else {
            let parent = out.parent().unwrap_or_else(|| Path::new(""));
            let stem = out
                .file_stem()
                .and_then(|s| s.to_str())
                .context("output path has no valid file stem")?;
            (parent, stem.to_string())
        }
    } else {
        (input_dir, format!("{input_stem}_subtitled"))
    };

    let srt = output_dir.join(format!("{out_stem}.{}.srt", cli.target_lang));
    let ass = output_dir.join(format!("{out_stem}.{}.ass", cli.target_lang));

    let video = match cli.mode {
        OutputMode::SubsOnly => None,
        OutputMode::Hardsub => {
            if let Some(ref out) = cli.output {
                if out.is_dir() {
                    Some(out.join(format!("{out_stem}.mp4")))
                } else {
                    Some(out.clone())
                }
            } else {
                Some(output_dir.join(format!("{out_stem}.mp4")))
            }
        }
        OutputMode::Softsub => {
            if let Some(ref out) = cli.output {
                if out.is_dir() {
                    let ext = cli
                        .input
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("mp4");
                    Some(out.join(format!("{out_stem}.{ext}")))
                } else {
                    Some(out.clone())
                }
            } else {
                let ext = cli
                    .input
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("mp4");
                Some(output_dir.join(format!("{out_stem}.{ext}")))
            }
        }
    };

    let input_canon = canonical_path(&cli.input);
    let sub_canon = cli.subtitle.as_ref().map(|s| canonical_path(s));

    let mut check_targets = Vec::new();
    if let Some(ref v) = video {
        check_targets.push(("video", v));
    }
    check_targets.push(("srt", &srt));
    check_targets.push(("ass", &ass));

    for (name, path) in check_targets {
        let canon = canonical_path(path);
        if canon == input_canon {
            anyhow::bail!(
                "output {} path canonically equals input file: {}",
                name,
                path.display()
            );
        }
        if let Some(ref sc) = sub_canon {
            if canon == *sc {
                anyhow::bail!(
                    "output {} path canonically equals subtitle input file: {}",
                    name,
                    path.display()
                );
            }
        }
    }

    Ok(OutputPaths { video, srt, ass })
}

/// Batches of BATCH_SIZE translated with up to `concurrency` in-flight requests;
/// each batch retried up to MAX_ATTEMPTS; after that Err("translation failed for lines a–b: ...").
pub async fn translate_all(
    t: &dyn Translator,
    segs: &mut [SubtitleSegment],
    src: &str,
    tgt: &str,
) -> anyhow::Result<()> {
    translate_all_with_progress(t, segs, src, tgt, 1, None::<fn(usize, usize)>).await
}

pub async fn translate_all_with_progress<F>(
    t: &dyn Translator,
    segs: &mut [SubtitleSegment],
    src: &str,
    tgt: &str,
    concurrency: usize,
    on_progress: Option<F>,
) -> anyhow::Result<()>
where
    F: FnMut(usize, usize) + Send,
{
    use futures_util::{stream, StreamExt};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    if segs.is_empty() {
        return Ok(());
    }

    let pb = ProgressBar::new(segs.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} lines ({eta})")
            .unwrap_or_else(|_| ProgressStyle::default_bar()),
    );

    let total_len = segs.len();
    // Chunks cloned out of segs so the futures can run concurrently
    // without holding overlapping &mut borrows.
    let jobs: Vec<(usize, usize, Vec<SubtitleSegment>)> = segs
        .chunks(BATCH_SIZE)
        .map(|c| {
            (
                c.first().map(|s| s.id).unwrap_or(1),
                c.last().map(|s| s.id).unwrap_or(1),
                c.to_vec(),
            )
        })
        .collect();

    // Shared progress state — must be Send because callers wrap this
    // future in tokio::select!/spawn (GUI cancel support).
    let done = Arc::new(AtomicUsize::new(0));
    let cb: Arc<Mutex<Option<F>>> = Arc::new(Mutex::new(on_progress));

    let results: Vec<(usize, usize, anyhow::Result<Vec<String>>)> = stream::iter(jobs)
        .map(|(start_line, end_line, chunk)| {
            let done = done.clone();
            let cb = cb.clone();
            let pb = pb.clone();
            async move {
                let res = translate_chunk(t, &chunk, src, tgt).await;
                if res.is_ok() {
                    let n = done.fetch_add(chunk.len(), Ordering::Relaxed) + chunk.len();
                    pb.inc(chunk.len() as u64);
                    if let Some(f) = cb.lock().unwrap().as_mut() {
                        f(n, total_len);
                    }
                }
                (start_line, end_line, res)
            }
        })
        .buffered(concurrency.max(1))
        .collect()
        .await;

    // .buffered preserves input order — apply results sequentially.
    let mut offset = 0usize;
    for (start_line, end_line, res) in results {
        match res {
            Ok(lines) => {
                let n = lines.len();
                for (seg, line) in segs[offset..offset + n].iter_mut().zip(lines) {
                    seg.translated_text = Some(line);
                }
                offset += n;
            }
            Err(err) => {
                pb.finish_and_clear();
                anyhow::bail!("translation failed for lines {start_line}–{end_line}: {err:#}");
            }
        }
    }

    pb.finish_and_clear();
    Ok(())
}

/// Translate one chunk with retry; on parse/validation failures halve the
/// batch until it succeeds or a single line still fails. Returns lines
/// aligned 1:1 with `chunk`.
async fn translate_chunk(
    t: &dyn Translator,
    chunk: &[SubtitleSegment],
    src: &str,
    tgt: &str,
) -> anyhow::Result<Vec<String>> {
    let mut out: Vec<Option<String>> = vec![None; chunk.len()];
    let mut parts: Vec<(&[SubtitleSegment], usize)> = vec![(chunk, 0)];

    'parts: while let Some((part, off)) = parts.pop() {
        let mut last_err = None;
        let mut fatal = false;
        let mut splittable = false;

        for attempt in 1..=MAX_ATTEMPTS {
            match t.translate_batch(part, src, tgt).await {
                Ok(translated_lines) => {
                    if translated_lines.len() != part.len() {
                        last_err = Some(anyhow::anyhow!(
                            "expected {} translated lines, got {}",
                            part.len(),
                            translated_lines.len()
                        ));
                        splittable = true;
                        continue;
                    }
                    for (dst, line) in out[off..off + part.len()].iter_mut().zip(translated_lines) {
                        *dst = Some(line);
                    }
                    continue 'parts;
                }
                Err(err) => {
                    let llm_err = err
                        .chain()
                        .find_map(|e| e.downcast_ref::<crate::llm::LlmError>())
                        .cloned();
                    last_err = Some(err);
                    match llm_err {
                        Some(le) if !le.is_retryable() => {
                            fatal = true;
                            break;
                        }
                        Some(le) => {
                            splittable = false;
                            if le.should_backoff() && attempt < MAX_ATTEMPTS {
                                tokio::time::sleep(std::time::Duration::from_secs(
                                    2 * attempt as u64,
                                ))
                                .await;
                            }
                        }
                        None => splittable = true,
                    }
                }
            }
        }

        if !fatal && splittable && part.len() > 1 {
            let mid = part.len() / 2;
            let (left, right) = part.split_at(mid);
            parts.push((right, off + mid));
            parts.push((left, off));
            continue 'parts;
        }

        return Err(last_err.unwrap_or_else(|| anyhow::anyhow!("unknown error")));
    }

    Ok(out.into_iter().map(|o| o.unwrap_or_default()).collect())
}

pub struct Deps<'a> {
    pub ffmpeg: Option<PathBuf>,
    pub transcriber: Option<&'a dyn Transcriber>,
    pub translator: Option<&'a dyn Translator>,
    pub tts: Option<&'a dyn crate::tts::TtsEngine>,
}

#[derive(Debug, Clone)]
pub struct Report {
    pub segments: usize,
    pub outputs: Vec<PathBuf>,
}

pub async fn run(cli: &Cli, deps: Deps<'_>) -> anyhow::Result<Report> {
    if !cli.input.exists() {
        anyhow::bail!("input file not found: {}", cli.input.display());
    }

    let output_paths = resolve_output_paths(cli)?;

    let temp_dir = tempfile::tempdir().context("failed to create temporary working directory")?;
    let (work_dir_buf, _temp_guard) = if cli.keep_temp {
        let path = temp_dir.keep();
        println!("Intermediate files kept at: {}", path.display());
        (path, None)
    } else {
        let path = temp_dir.path().to_path_buf();
        (path, Some(temp_dir))
    };
    let work_dir = work_dir_buf.as_path();

    let mut segments = if let Some(ref sub_path) = cli.subtitle {
        if !sub_path.exists() {
            anyhow::bail!("subtitle input file not found: {}", sub_path.display());
        }
        let sp = ProgressBar::new_spinner();
        sp.set_message(format!("Parsing subtitle file {}...", sub_path.display()));
        sp.enable_steady_tick(std::time::Duration::from_millis(100));
        let segs = crate::subtitle::parser::parse_subtitle_file(sub_path)?;
        sp.finish_and_clear();
        segs
    } else {
        let ffmpeg = deps
            .ffmpeg
            .as_ref()
            .context("ffmpeg is required for audio extraction")?;
        let transcriber = deps
            .transcriber
            .context("transcriber is required when no subtitle file is provided")?;

        let (audio_path, cmd) = match cli.stt_engine {
            SttEngine::Local => {
                let wav = work_dir.join("audio.wav");
                let c = crate::media::build_extract_wav_command(ffmpeg, &cli.input, &wav);
                (wav, c)
            }
            SttEngine::Api => {
                let mp3 = work_dir.join("audio.mp3");
                let c = crate::media::build_extract_mp3_command(ffmpeg, &cli.input, &mp3);
                (mp3, c)
            }
        };

        let sp = ProgressBar::new_spinner();
        sp.set_message("Extracting audio from input video...");
        sp.enable_steady_tick(std::time::Duration::from_millis(100));
        crate::media::run_ffmpeg(cmd, "audio extraction")?;
        sp.finish_and_clear();

        let sp = ProgressBar::new_spinner();
        sp.set_message("Transcribing audio...");
        sp.enable_steady_tick(std::time::Duration::from_millis(100));
        let segs = transcriber.transcribe(&audio_path).await?;
        sp.finish_and_clear();

        if segs.is_empty() {
            anyhow::bail!("No speech detected in {}", cli.input.display());
        }
        segs
    };

    if !cli.skip_translate {
        let translator = deps
            .translator
            .context("translator is required when --skip-translate is not set")?;
        let sp = ProgressBar::new_spinner();
        sp.set_message(format!(
            "Translating {} subtitle segments...",
            segments.len()
        ));
        sp.enable_steady_tick(std::time::Duration::from_millis(100));
        translate_all_with_progress(
            translator,
            &mut segments,
            &cli.source_lang,
            &cli.target_lang,
            cli.translate_concurrency,
            None::<fn(usize, usize)>,
        )
        .await?;
        sp.finish_and_clear();
    }

    let input_stem = cli
        .input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video");

    let srt_content = crate::subtitle::srt::render_srt(&segments, cli.bilingual);
    let ass_content = crate::subtitle::ass::render_ass(&segments, cli.bilingual, input_stem);

    let work_srt = work_dir.join("subtitle.srt");
    let work_ass = work_dir.join("subtitle.ass");

    std::fs::write(&work_srt, srt_content)
        .with_context(|| format!("failed to write intermediate srt at {}", work_srt.display()))?;
    std::fs::write(&work_ass, ass_content)
        .with_context(|| format!("failed to write intermediate ass at {}", work_ass.display()))?;

    let dub_track: Option<(PathBuf, bool)> = if cli.dub {
        let engine = deps.tts.context("tts engine required for --dub")?;
        let ffmpeg = deps
            .ffmpeg
            .as_ref()
            .context("ffmpeg is required for --dub")?;
        let sp = ProgressBar::new_spinner();
        sp.set_message("Dubbing translated subtitles...");
        sp.enable_steady_tick(std::time::Duration::from_millis(100));
        let sp_cb = sp.clone(); // ProgressBar is a shared handle
        let seg_total = segments.len();
        let wav_and_audio = crate::dub::render_dub_track(
            engine,
            &segments,
            ffmpeg,
            work_dir,
            &cli.input,
            move |done, _t| sp_cb.set_message(format!("Đang lồng tiếng {done}/{seg_total}...")),
            || false, // CLI has no cancellation; `FnMut` bound accepts plain fns
        )
        .await?;
        sp.finish_and_clear();
        Some(wav_and_audio)
    } else {
        None
    };

    let mut outputs = Vec::new();

    if let Some(ref video_dest) = output_paths.video {
        if let Some(parent) = video_dest.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).with_context(|| {
                    format!("failed to create output directory {}", parent.display())
                })?;
            }
        }
    }
    if let Some(parent) = output_paths.srt.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("failed to create output directory {}", parent.display())
            })?;
        }
    }

    match cli.mode {
        OutputMode::Hardsub => {
            let ffmpeg = deps
                .ffmpeg
                .as_ref()
                .context("ffmpeg is required for hardsub")?;
            let video_dest = output_paths
                .video
                .as_ref()
                .context("output video path is required for hardsub")?;

            let sp = ProgressBar::new_spinner();
            sp.set_message("Burning subtitles into video (hardsub)...");
            sp.enable_steady_tick(std::time::Duration::from_millis(100));

            let enc = crate::media::EncoderOpts {
                nvenc: !cli.no_gpu && crate::media::nvenc_available(ffmpeg),
                speed: crate::media::RenderSpeed::parse(&cli.render_speed),
                crf: cli.crf,
            };

            // --pad-subs: probe frame size, write a padded-layout ASS for the
            // burn so subs land in the added black band below the picture.
            // The exported .ass side file keeps the standard layout.
            let (burn_ass, pad_px) = match cli.pad_subs {
                Some(px) => {
                    let (w, h) =
                        crate::media::probe_video_size(ffmpeg, &cli.input).unwrap_or((1920, 1080));
                    let pad = if px == 0 {
                        crate::subtitle::ass::auto_sub_bar_height(h, cli.bilingual)
                    } else {
                        px & !1
                    };
                    let layout = crate::subtitle::ass::sub_bar_layout(w, h, pad, cli.bilingual);
                    let padded = work_dir.join("subtitle_pad.ass");
                    std::fs::write(
                        &padded,
                        crate::subtitle::ass::render_ass_with_layout(
                            &segments,
                            cli.bilingual,
                            input_stem,
                            &layout,
                        ),
                    )
                    .with_context(|| {
                        format!("failed to write padded ass at {}", padded.display())
                    })?;
                    (padded, pad)
                }
                None => (work_ass.clone(), 0),
            };

            let cmd = if let Some((ref dw, has_audio)) = dub_track {
                crate::media::build_hardsub_dub_command(
                    ffmpeg,
                    &cli.input,
                    &burn_ass,
                    dw,
                    video_dest,
                    cli.dub_orig_volume,
                    has_audio,
                    pad_px,
                    enc,
                )?
            } else {
                crate::media::build_hardsub_command(
                    ffmpeg, &cli.input, &burn_ass, video_dest, pad_px, enc,
                )?
            };
            crate::media::run_ffmpeg(cmd, "hardsub video burn")?;
            sp.finish_and_clear();
            outputs.push(video_dest.clone());

            if cli.keep_temp {
                std::fs::copy(&work_srt, &output_paths.srt).with_context(|| {
                    format!("failed to copy srt to {}", output_paths.srt.display())
                })?;
                std::fs::copy(&work_ass, &output_paths.ass).with_context(|| {
                    format!("failed to copy ass to {}", output_paths.ass.display())
                })?;
                outputs.push(output_paths.srt.clone());
                outputs.push(output_paths.ass.clone());
            }
        }
        OutputMode::Softsub => {
            let ffmpeg = deps
                .ffmpeg
                .as_ref()
                .context("ffmpeg is required for softsub")?;
            let video_dest = output_paths
                .video
                .as_ref()
                .context("output video path is required for softsub")?;

            let sp = ProgressBar::new_spinner();
            sp.set_message("Embedding subtitles into video container (softsub)...");
            sp.enable_steady_tick(std::time::Duration::from_millis(100));

            let cmd = if let Some((ref dw, has_audio)) = dub_track {
                crate::media::build_softsub_dub_command(
                    ffmpeg,
                    &cli.input,
                    &work_srt,
                    dw,
                    video_dest,
                    cli.dub_orig_volume,
                    has_audio,
                )?
            } else {
                crate::media::build_softsub_command(ffmpeg, &cli.input, &work_srt, video_dest)?
            };
            crate::media::run_ffmpeg(cmd, "softsub video mux")?;
            sp.finish_and_clear();
            outputs.push(video_dest.clone());

            if cli.keep_temp {
                std::fs::copy(&work_srt, &output_paths.srt).with_context(|| {
                    format!("failed to copy srt to {}", output_paths.srt.display())
                })?;
                std::fs::copy(&work_ass, &output_paths.ass).with_context(|| {
                    format!("failed to copy ass to {}", output_paths.ass.display())
                })?;
                outputs.push(output_paths.srt.clone());
                outputs.push(output_paths.ass.clone());
            }
        }
        OutputMode::SubsOnly => {
            let sp = ProgressBar::new_spinner();
            sp.set_message("Exporting subtitles...");
            sp.enable_steady_tick(std::time::Duration::from_millis(100));

            std::fs::copy(&work_srt, &output_paths.srt)
                .with_context(|| format!("failed to copy srt to {}", output_paths.srt.display()))?;
            std::fs::copy(&work_ass, &output_paths.ass)
                .with_context(|| format!("failed to copy ass to {}", output_paths.ass.display()))?;
            sp.finish_and_clear();
            outputs.push(output_paths.srt.clone());
            outputs.push(output_paths.ass.clone());

            if let Some((ref dw, _)) = dub_track {
                let wav_dest = output_paths.srt.with_file_name(format!(
                    "{}.dub.wav",
                    output_paths
                        .srt
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("out")
                ));
                std::fs::copy(dw, &wav_dest)
                    .with_context(|| format!("failed to copy dub wav to {}", wav_dest.display()))?;
                outputs.push(wav_dest);
            }
        }
    }

    Ok(Report {
        segments: segments.len(),
        outputs,
    })
}
