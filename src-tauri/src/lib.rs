mod commands;
mod core;
mod scraper;
mod templates;
mod webview_fetch;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(webview_fetch::PendingCapture::default())
        .manage(webview_fetch::PendingProbe::default())
        .manage(commands::NovelIndexCache::default())
        .manage(commands::DownloadControl::default())
        .invoke_handler(tauri::generate_handler![
            commands::select_working_folder,
            commands::list_novel_chapters,
            commands::cancel_download,
            commands::download_novel_chapters,
            commands::download_novel,
            webview_fetch::open_fetch_window,
            webview_fetch::probe_fetcher_page,
            webview_fetch::submit_probe_result,
            webview_fetch::submit_captured_html,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
