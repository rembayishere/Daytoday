mod models;
mod storage;
mod commands;
mod crypto;
mod webdav;

use commands::{AppState, start_auto_sync};
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data = storage::load(app.handle());
            let state = AppState(Mutex::new(data));
            let handle = app.handle().clone();
            app.manage(state);
            start_auto_sync(handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_all_data,
            commands::add_note,
            commands::delete_note,
            commands::update_note,
            commands::add_clip,
            commands::delete_clip,
            commands::update_clip,
            commands::add_question,
            commands::update_question,
            commands::delete_question,
            commands::cycle_question,
            commands::update_question_note,
            commands::add_task_subtask,
            commands::toggle_task_subtask,
            commands::delete_task_subtask,
            commands::add_flashcard,
            commands::delete_flashcard,
            commands::update_flashcard,
            commands::add_task,
            commands::update_task,
            commands::delete_task,
            commands::move_task,
            commands::add_attachment,
            commands::add_attachment_from_path,
            commands::delete_attachment,
            commands::check_attachment_files,
            commands::link_note_attachment,
            commands::unlink_note_attachment,
            commands::move_attachment,
            commands::create_attachment_folder,
            commands::rename_attachment_folder,
            commands::delete_attachment_folder,
            commands::open_attachment_folder,
            commands::open_attachment_folder_by_name,
            commands::list_remote_attachments,
            commands::debug_list_remote_attachments,
            commands::verify_attachment_encryption,
            commands::upload_attachment,
            commands::download_attachment,
            commands::read_file,
            commands::save_attachment_dir,
            commands::save_ai_config,
            commands::test_ai_connection,
            commands::list_ai_models,
            commands::save_webdav_config,
            commands::save_shortcuts,
            commands::test_webdav,
            commands::verify_webdav_encryption,
            commands::sync_webdav,
            commands::sync_pull,
            commands::save_data_dir,
            commands::save_attachment_move_mode,
            commands::delete_file,
            commands::open_file_explorer,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
