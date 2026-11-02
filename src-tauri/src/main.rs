pub mod commands;
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
            pick_save_file,
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
