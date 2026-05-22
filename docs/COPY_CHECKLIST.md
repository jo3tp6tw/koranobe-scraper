# 從 my-tauri-app 複製檔案清單

> **一鍵同步：** 在 `newtoki-scraper` 目錄執行  
> `powershell -ExecutionPolicy Bypass -File .\scripts\sync-from-my-tauri-app.ps1`

來源根目錄：`../my-tauri-app/`  
目標根目錄：`./`（本專案）

---

## 一、Rust 核心（必複製）

| 來源 | 目標 |
|------|------|
| `src-tauri/src/webview_fetch.rs` | `src-tauri/src/webview_fetch.rs` |
| `src-tauri/src/commands.rs` | `src-tauri/src/commands.rs` |
| `src-tauri/src/core/error.rs` | `src-tauri/src/core/error.rs` |
| `src-tauri/src/core/novel_storage.rs` | `src-tauri/src/core/novel_storage.rs` |
| `src-tauri/src/core/mod.rs` | `src-tauri/src/core/mod.rs`（複製後刪掉 `pub mod storage;` 若不需要） |
| `src-tauri/src/scraper/` **整個資料夾** | `src-tauri/src/scraper/` |
| `src-tauri/src/templates/mod.rs` | `src-tauri/src/templates/mod.rs` |
| `src-tauri/src/templates/schema.rs` | `src-tauri/src/templates/schema.rs` |
| `src-tauri/src/templates/built_in.rs` | `src-tauri/src/templates/built_in.rs` |
| `src-tauri/src/templates/registry.rs` | `src-tauri/src/templates/registry.rs` |
| `src-tauri/src/templates/user.rs` | `src-tauri/src/templates/user.rs` |
| `src-tauri/templates/newtoki.json` | `src-tauri/templates/newtoki.json` |

### 不必複製

- `src-tauri/src/ai/` — 未来功能
- `src-tauri/templates/kakuyomu.json`、`syosetu.json` — 若只做 newtoki
- `src-tauri/src/core/storage.rs` — novel_storage 未使用

---

## 二、複製後要改的地方

### 1. `src-tauri/src/templates/built_in.rs`

只保留 newtoki：

```rust
const NEWTOKI_JSON: &str = include_str!("../../templates/newtoki.json");

pub fn get(name: &str) -> Option<Result<Template>> {
    let raw = match name {
        "newtoki" => NEWTOKI_JSON,
        _ => return None,
    };
    Some(Template::from_json_str(raw))
}
```

（測試區塊可刪或只留 `loads_newtoki`）

### 2. `src-tauri/src/lib.rs`

替換成（或參考 my-tauri-app 的 lib.rs，去掉 greet / test_scrape / scrape_and_save）：

```rust
mod commands;
mod core;
mod scraper;
mod templates;
mod webview_fetch;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(webview_fetch::PendingCapture::default())
        .manage(webview_fetch::PendingProbe::default())
        .manage(webview_fetch::PendingNetLog::default())
        .invoke_handler(tauri::generate_handler![
            commands::select_working_folder,
            commands::download_novel,
            webview_fetch::open_fetch_window,
            webview_fetch::probe_content,
            webview_fetch::capture_html,
            webview_fetch::submit_captured_html,
            webview_fetch::submit_content_probe,
            webview_fetch::close_fetch_window,
            webview_fetch::save_html_to_file,
            webview_fetch::save_probe_report,
            webview_fetch::dump_net_log,
            webview_fetch::submit_net_log,
            webview_fetch::inspect_selection,
            webview_fetch::extract_chapter_text,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 3. `src-tauri/src/core/mod.rs`（若從原專案複製）

```rust
pub mod error;
pub mod novel_storage;
// 刪掉: pub mod storage;
```

### 4. `src-tauri/src/scraper/mod.rs`（可選精簡）

發布版可刪掉 `extractor` 相關（lib 已不再 invoke test_scrape）：

```rust
pub mod engine;
pub mod http;
pub mod nextdata;
pub mod selector;
// 刪掉: pub mod extractor;
```

---

## 三、前端（必複製，再精簡）

| 來源 | 目標 | 說明 |
|------|------|------|
| `src/App.tsx` | `src/App.tsx` | 複製後刪掉 CF 診斷區（3b/3c/3d/4/2），只保留下載流程 |
| `src/App.css` | `src/App.css` | 依 UI 調整 |

`src/main.tsx` 已就緒，不必改。

---

## 四、文件（可選）

| 來源 | 目標 |
|------|------|
| `../newtoki.notes.md` | `docs/newtoki.notes.md` |
| `../newtoki-index` | `docs/fixtures/newtoki-index`（給 `engine/mod.rs` 測試用，可選） |
| `../json.md` | `docs/json.md`（可選） |
| `../COPY_CHECKLIST.md` | `docs/COPY_CHECKLIST.md`（可選） |

探測報告（`newtoki-probe-*.json` / `scraper-probe-*.json`）請放到 `docs/probes/`，已在 `.gitignore` 排除。

---

## 五、複製完成後

```powershell
cd newtoki-scraper
npm install
npm run tauri dev
```

若 `cargo test` 要跑 newtoki fixture，需把 `newtoki-index` 放在 `docs/fixtures/`（見 `engine/mod.rs` 的 `include_str!` 路徑）。
