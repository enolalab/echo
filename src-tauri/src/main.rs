pub mod commands;
pub mod state;
pub mod types;

use commands::*;
use state::AppState;

fn main() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            get_system_info,
            download_whisper_model,
            start_pipeline,
            cancel_pipeline,
            export_subtitles,
            open_in_file_manager
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
