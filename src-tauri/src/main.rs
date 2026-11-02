pub mod commands;
pub mod media_server;
pub mod settings_commands;
pub mod skill_commands;
pub mod state;
pub mod types;
pub mod youtube_commands;

use commands::*;
use settings_commands::*;
use skill_commands::*;
use state::AppState;
use youtube_commands::*;

fn main() {
    // Linux/WebKitGTK: <video> playback is routed through GStreamer, which has
    // no handler for the `asset://` scheme produced by convertFileSrc —
    // resulting in MEDIA_ERR_SRC_NOT_SUPPORTED (error 4). WebKitGTK >= 2.38
    // allows whitelisting extra URI protocols via this env var.
    #[cfg(target_os = "linux")]
    match std::env::var("WEBKIT_GST_ALLOWED_URI_PROTOCOLS") {
        Ok(existing) if !existing.split(':').any(|p| p == "asset") => {
            std::env::set_var(
                "WEBKIT_GST_ALLOWED_URI_PROTOCOLS",
                format!("{existing}:asset"),
            );
        }
        Err(_) => std::env::set_var("WEBKIT_GST_ALLOWED_URI_PROTOCOLS", "asset"),
        _ => {}
    }

    // Localhost media server for <video> playback (asset:// can't be serviced
    // by WebKitGTK's GStreamer pipeline).
    media_server::start();

    let settings = auto_subtitle::settings::SettingsStore::open_default()
        .expect("Không mở được thư mục cài đặt");
    tauri::Builder::default()
        .manage(AppState::new(settings))
        .invoke_handler(tauri::generate_handler![
            get_system_info,
            download_whisper_model,
            start_pipeline,
            cancel_pipeline,
            export_subtitles,
            open_in_file_manager,
            pick_video_file,
            pick_video_files,
            get_output_dir,
            pick_directory,
            pick_save_file,
            media_server::get_media_url,
            get_settings,
            save_profile,
            delete_profile,
            set_default_profile,
            save_general,
            set_stt_key,
            test_profile,
            list_profile_models,
            save_skill,
            delete_skill,
            duplicate_skill,
            set_default_skill,
            import_skill,
            export_skill,
            try_skill,
            check_yt_dlp_status,
            ensure_yt_dlp,
            inspect_youtube_url,
            download_youtube_items,
            cancel_youtube_download
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
