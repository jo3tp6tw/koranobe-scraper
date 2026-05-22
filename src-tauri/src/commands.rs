// Tauri commands for the novel downloader (template-driven).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;

use crate::core::novel_storage::NovelStorage;
use crate::scraper::engine::{gate, novel as engine_novel, Engine};
use crate::templates::schema::GateRule;
use crate::templates::{self, Chapter, NovelInfo};
use crate::webview_fetch::{self, CaptureResult, WaitMode};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "stage")]
pub enum ProgressEvent {
    Start { url: String },
    FetchingIndex,
    WaitingCloudflare { url: String },
    NovelInfo { info: NovelInfo },
    /// 下載開始前：預先列出將跳過的章節（顯示在章節進度之前）
    SkipPreview { items: Vec<SkipPreviewItem> },
    Chapter {
        chapter: ChapterProgress,
    },
    Done { folder: String },
    Cancelled { folder: String, message: String },
    Error { message: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkipPreviewItem {
    pub index: usize,
    pub title: String,
    pub id: String,
    pub reason: SkipReason,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SkipReason {
    AlreadyExists,
    LoginRequired,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterProgress {
    pub index: usize,
    pub total: usize,
    pub title: String,
    pub id: String,
    pub char_count: usize,
    pub status: ChapterStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ChapterStatus {
    Skipped,
    Downloaded,
    /// Skipped: template `gates` matched (login / paid / etc.).
    LoginRequired,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LocalChapterStatus {
    None,
    Exists,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterListItem {
    pub index: usize,
    pub id: String,
    pub title: String,
    pub url: String,
    pub requires_login: bool,
    pub local_status: LocalChapterStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListNovelChaptersResponse {
    pub title: String,
    pub author: String,
    pub introduction: String,
    pub tags: Vec<String>,
    pub chapters: Vec<ChapterListItem>,
    /// Template default; UI can use as initial chapter delay.
    pub default_chapter_delay_ms: u64,
}

#[derive(Default)]
pub struct DownloadControl {
    cancel: AtomicBool,
}

impl DownloadControl {
    pub fn reset(&self) {
        self.cancel.store(false, Ordering::SeqCst);
    }

    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
}

#[derive(Default)]
pub struct NovelIndexCache {
    inner: Mutex<Option<CachedNovelIndex>>,
}

#[derive(Debug, Clone)]
struct CachedNovelIndex {
    url: String,
    template_name: String,
    info: NovelInfo,
    index_html: String,
}

const PROGRESS_EVENT: &str = "scrape-progress";

fn emit(app: &AppHandle, evt: ProgressEvent) {
    if let Err(e) = app.emit(PROGRESS_EVENT, evt) {
        eprintln!("emit progress failed: {}", e);
    }
}

fn resolve_chapter_delay_ms(override_ms: Option<u64>, template_default: u64) -> u64 {
    override_ms.unwrap_or(template_default)
}

fn check_cancel(
    cancel: &DownloadControl,
    app: &AppHandle,
    folder: &str,
) -> Result<(), String> {
    if cancel.is_cancelled() {
        let msg = "使用者已停止下載".to_string();
        emit(
            app,
            ProgressEvent::Cancelled {
                folder: folder.to_string(),
                message: msg.clone(),
            },
        );
        Err(msg)
    } else {
        Ok(())
    }
}

async fn interruptible_delay(cancel: &DownloadControl, delay_ms: u64) -> Result<(), String> {
    if delay_ms == 0 {
        return Ok(());
    }
    const STEP_MS: u64 = 100;
    let mut remaining = delay_ms;
    while remaining > 0 {
        if cancel.is_cancelled() {
            return Err("使用者已停止下載".into());
        }
        let wait = remaining.min(STEP_MS);
        tokio::time::sleep(Duration::from_millis(wait)).await;
        remaining -= wait;
    }
    Ok(())
}

fn load_engine(template_name: &str) -> Result<Engine, String> {
    let template = templates::built_in::get(template_name)
        .ok_or_else(|| format!("未知的模板: {}", template_name))?
        .map_err(|e| e.to_string())?;
    Engine::new(template).map_err(|e| e.to_string())
}

fn cache_index(
    cache: &NovelIndexCache,
    url: &str,
    template_name: &str,
    info: NovelInfo,
    index_html: String,
) {
    *cache.inner.lock().unwrap() = Some(CachedNovelIndex {
        url: url.to_string(),
        template_name: template_name.to_string(),
        info,
        index_html,
    });
}

fn take_cached_index(
    cache: &NovelIndexCache,
    url: &str,
    template_name: &str,
) -> Option<CachedNovelIndex> {
    let mut guard = cache.inner.lock().unwrap();
    match guard.as_ref() {
        Some(c) if c.url == url && c.template_name == template_name => guard.clone(),
        _ => None,
    }
}

#[tauri::command]
pub async fn select_working_folder(app: AppHandle) -> Result<Option<String>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel::<Option<PathBuf>>();
    app.dialog().file().pick_folder(move |folder| {
        let path = folder.and_then(|f| f.into_path().ok());
        let _ = tx.send(path);
    });
    let folder = rx.await.map_err(|e| e.to_string())?;
    Ok(folder.map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn list_novel_chapters(
    app: AppHandle,
    cache: State<'_, NovelIndexCache>,
    url: String,
    working_folder: Option<String>,
    template_name: Option<String>,
) -> Result<ListNovelChaptersResponse, String> {
    let template_name = template_name.unwrap_or_else(|| "newtoki".to_string());
    let engine = load_engine(&template_name)?;

    emit(&app, ProgressEvent::FetchingIndex);
    let (info, index_html) = fetch_novel_index_from_webview(&app, &engine, &url).await?;

    cache_index(
        &cache,
        &url,
        &template_name,
        info.clone(),
        index_html,
    );

    Ok(build_list_response(
        &info,
        working_folder.as_deref(),
        engine.template.fetch.request_delay_ms,
    ))
}

#[tauri::command]
pub fn cancel_download(cancel: State<'_, DownloadControl>) -> Result<(), String> {
    cancel.request_cancel();
    Ok(())
}

#[tauri::command]
pub async fn download_novel_chapters(
    app: AppHandle,
    cache: State<'_, NovelIndexCache>,
    cancel: State<'_, DownloadControl>,
    url: String,
    working_folder: String,
    chapter_ids: Vec<String>,
    template_name: Option<String>,
    force: Option<bool>,
    chapter_delay_ms: Option<u64>,
) -> Result<String, String> {
    if chapter_ids.is_empty() {
        return Err("請至少選擇一個章節".into());
    }

    cancel.reset();
    let template_name = template_name.unwrap_or_else(|| "newtoki".to_string());
    let force = force.unwrap_or(false);
    let chapter_id_set: HashSet<String> = chapter_ids.into_iter().collect();

    emit(&app, ProgressEvent::Start { url: url.clone() });

    let engine = load_engine(&template_name).map_err(|e| {
        emit(&app, ProgressEvent::Error { message: e.clone() });
        e
    })?;
    let chapter_delay_ms =
        resolve_chapter_delay_ms(chapter_delay_ms, engine.template.fetch.request_delay_ms);

    let cached = take_cached_index(&cache, &url, &template_name);
    let (info, index_html) = if let Some(cached) = cached {
        (cached.info, cached.index_html)
    } else {
        emit(&app, ProgressEvent::FetchingIndex);
        fetch_novel_index_from_webview(&app, &engine, &url).await?
    };

    download_chapters_via_webview(
        &app,
        &engine,
        &cancel,
        &url,
        &working_folder,
        &info,
        &index_html,
        Some(&chapter_id_set),
        force,
        chapter_delay_ms,
    )
    .await
}

#[tauri::command]
pub async fn download_novel(
    app: AppHandle,
    cancel: State<'_, DownloadControl>,
    url: String,
    working_folder: String,
    template_name: Option<String>,
    force: Option<bool>,
    chapter_delay_ms: Option<u64>,
) -> Result<String, String> {
    cancel.reset();
    let template_name = template_name.unwrap_or_else(|| "newtoki".to_string());
    let force = force.unwrap_or(false);

    emit(&app, ProgressEvent::Start { url: url.clone() });

    let engine = load_engine(&template_name).map_err(|e| {
        let msg = e.to_string();
        emit(&app, ProgressEvent::Error { message: msg.clone() });
        msg
    })?;
    let chapter_delay_ms =
        resolve_chapter_delay_ms(chapter_delay_ms, engine.template.fetch.request_delay_ms);

    emit(&app, ProgressEvent::FetchingIndex);
    let (info, index_html) = fetch_novel_index_from_webview(&app, &engine, &url).await?;

    download_chapters_via_webview(
        &app,
        &engine,
        &cancel,
        &url,
        &working_folder,
        &info,
        &index_html,
        None,
        force,
        chapter_delay_ms,
    )
    .await
}

fn build_list_response(
    info: &NovelInfo,
    working_folder: Option<&str>,
    default_chapter_delay_ms: u64,
) -> ListNovelChaptersResponse {
    let local_dir = working_folder.map(|root| {
        let storage = NovelStorage::new(root);
        storage.novel_dir(&info.title)
    });

    let chapters = info
        .chapters
        .iter()
        .map(|chapter| {
            let local_status = match local_dir.as_ref() {
                Some(dir) if dir.exists() => {
                    let storage = NovelStorage::new(working_folder.unwrap_or(""));
                    if storage.chapter_exists(dir, chapter) {
                        LocalChapterStatus::Exists
                    } else {
                        LocalChapterStatus::None
                    }
                }
                _ => LocalChapterStatus::None,
            };
            ChapterListItem {
                index: chapter.index,
                id: chapter.id.clone(),
                title: chapter.title.clone(),
                url: chapter.url.clone(),
                requires_login: chapter.requires_login,
                local_status,
            }
        })
        .collect();

    ListNovelChaptersResponse {
        title: info.title.clone(),
        author: info.author.clone(),
        introduction: info.introduction.clone(),
        tags: info.tags.clone(),
        chapters,
        default_chapter_delay_ms,
    }
}

async fn fetch_novel_index_from_webview(
    app: &AppHandle,
    engine: &Engine,
    url: &str,
) -> Result<(NovelInfo, String), String> {
    let index_cap = webview_fetch::fetch_page(app, url, WaitMode::Index, false)
        .await
        .map_err(|e| emit_err(app, e))?;

    if index_cap.wait_status == "timeout" {
        return Err(emit_err(
            app,
            "目錄頁載入超時：請在抓取視窗完成 Cloudflare 驗證後重試".into(),
        ));
    }

    let mut info = engine_novel::extract_novel_info_from_html(engine, url, &index_cap.html)
        .map_err(|e| emit_err(app, e.to_string()))?;

    let index_row_selector = gate::index_chapters_row_selector(&engine.template);
    if let Some(index_gate) = engine
        .template
        .gates
        .as_ref()
        .and_then(|g| g.index_item.as_ref())
    {
        for chapter in &mut info.chapters {
            if gate::index_episode_gated(
                &index_cap.html,
                &chapter.id,
                index_gate,
                index_row_selector,
            ) {
                chapter.requires_login = true;
            }
        }
    }

    if info.chapters.is_empty() {
        return Err(emit_err(
            app,
            "目錄頁未解析到章節列表，請確認 URL 與 Cloudflare 驗證".into(),
        ));
    }

    emit(
        app,
        ProgressEvent::NovelInfo {
            info: info.clone(),
        },
    );

    Ok((info, index_cap.html))
}

async fn download_chapters_via_webview(
    app: &AppHandle,
    engine: &Engine,
    cancel: &DownloadControl,
    url: &str,
    working_folder: &str,
    info: &NovelInfo,
    index_html: &str,
    chapter_ids: Option<&HashSet<String>>,
    force: bool,
    chapter_delay_ms: u64,
) -> Result<String, String> {
    let selected: Vec<&Chapter> = info
        .chapters
        .iter()
        .filter(|c| chapter_ids.map_or(true, |ids| ids.contains(&c.id)))
        .collect();

    if selected.is_empty() {
        return Err(emit_err(app, "找不到所選章節，請重新列出章節".into()));
    }

    let storage = NovelStorage::new(working_folder);
    let novel_dir = storage
        .init_novel(info)
        .map_err(|e| emit_err(app, e.to_string()))?;
    storage
        .save_intro(&novel_dir, info)
        .map_err(|e| emit_err(app, e.to_string()))?;

    let total = selected.len();
    let needs_chapters = selected.iter().any(|c| {
        force || !storage.chapter_exists(&novel_dir, c)
    });

    if needs_chapters {
        let hook_url = selected
            .iter()
            .find(|c| force || !storage.chapter_exists(&novel_dir, c))
            .map(|c| c.url.clone())
            .unwrap_or_else(|| url.to_string());
        webview_fetch::recreate_fetcher_with_hook(app.clone(), hook_url)
            .await
            .map_err(|e| emit_err(app, e))?;
        let chapter_gate = engine
            .template
            .gates
            .as_ref()
            .and_then(|g| g.chapter.as_ref());
        let _ = webview_fetch::set_chapter_gate_selector(app, chapter_gate);
    }

    let index_gate = engine
        .template
        .gates
        .as_ref()
        .and_then(|g| g.index_item.as_ref());
    let index_row_selector = gate::index_chapters_row_selector(&engine.template);
    let chapter_gate = engine
        .template
        .gates
        .as_ref()
        .and_then(|g| g.chapter.as_ref());

    let skip_preview = collect_skip_preview(
        &selected,
        &storage,
        &novel_dir,
        index_html,
        index_gate,
        index_row_selector,
        force,
    );
    emit(
        app,
        ProgressEvent::SkipPreview {
            items: skip_preview,
        },
    );

    for chapter in selected {
        if let Err(e) = check_cancel(cancel, app, &novel_dir.to_string_lossy()) {
            return Err(e);
        }

        if !force && storage.chapter_exists(&novel_dir, chapter) {
            let chars = storage.chapter_char_count(&novel_dir, chapter);
            emit_chapter(app, chapter, total, ChapterStatus::Skipped, chars);
            continue;
        }

        if chapter_requires_skip_on_index(chapter, index_html, index_gate, index_row_selector) {
            eprintln!(
                "[{}] {} | {} | {} | 0 | skipped (index gate)",
                engine.template.name,
                format!("{:04}", chapter.index),
                chapter.title,
                chapter.id
            );
            emit_chapter(app, chapter, total, ChapterStatus::LoginRequired, 0);
            continue;
        }

        match fetch_and_save_chapter_webview(
            app, engine, url, chapter, &storage, &novel_dir, chapter_gate,
        )
        .await
        {
            Ok(chars) => emit_chapter(app, chapter, total, ChapterStatus::Downloaded, chars),
            Err(ChapterFetchError::LoginRequired) => {
                emit_chapter(app, chapter, total, ChapterStatus::LoginRequired, 0);
            }
            Err(ChapterFetchError::Failed(msg)) => {
                emit_chapter(app, chapter, total, ChapterStatus::Failed, 0);
                emit(app, ProgressEvent::Error { message: msg });
            }
        }

        if chapter_delay_ms > 0 {
            if let Err(e) = interruptible_delay(cancel, chapter_delay_ms).await {
                check_cancel(cancel, app, &novel_dir.to_string_lossy())?;
                return Err(e);
            }
        }
    }

    let folder_str = novel_dir.to_string_lossy().to_string();
    emit(
        app,
        ProgressEvent::Done {
            folder: folder_str.clone(),
        },
    );
    Ok(folder_str)
}

enum ChapterFetchError {
    LoginRequired,
    Failed(String),
}

fn collect_skip_preview(
    chapters: &[&Chapter],
    storage: &NovelStorage,
    novel_dir: &Path,
    index_html: &str,
    index_gate: Option<&GateRule>,
    index_row_selector: Option<&str>,
    force: bool,
) -> Vec<SkipPreviewItem> {
    let mut out = Vec::new();
    for chapter in chapters {
        let reason = if !force && storage.chapter_exists(novel_dir, chapter) {
            Some(SkipReason::AlreadyExists)
        } else if chapter_requires_skip_on_index(
            chapter,
            index_html,
            index_gate,
            index_row_selector,
        ) {
            Some(SkipReason::LoginRequired)
        } else {
            None
        };
        if let Some(reason) = reason {
            out.push(SkipPreviewItem {
                index: chapter.index,
                title: chapter.title.clone(),
                id: chapter.id.clone(),
                reason,
            });
        }
    }
    out
}

fn chapter_requires_skip_on_index(
    chapter: &Chapter,
    index_html: &str,
    index_gate: Option<&GateRule>,
    index_row_selector: Option<&str>,
) -> bool {
    if chapter.requires_login {
        return true;
    }
    index_gate
        .map(|r| gate::index_episode_gated(index_html, &chapter.id, r, index_row_selector))
        .unwrap_or(false)
}

async fn fetch_and_save_chapter_webview(
    app: &AppHandle,
    engine: &Engine,
    input_url: &str,
    chapter: &Chapter,
    storage: &NovelStorage,
    novel_dir: &Path,
    chapter_gate: Option<&GateRule>,
) -> Result<usize, ChapterFetchError> {
    let cap = webview_fetch::fetch_page(app, &chapter.url, WaitMode::Chapter, true)
        .await
        .map_err(|e| {
            ChapterFetchError::Failed(format!("第 {} 章 WebView 抓取失敗: {}", chapter.index, e))
        })?;

    if webview_fetch::capture_is_gated(&cap, chapter_gate) {
        let _ = storage.remove_chapter(novel_dir, chapter);
        eprintln!(
            "[{}] chapter {} skipped (chapter gate): {}",
            engine.template.name, chapter.index, chapter.title
        );
        return Err(ChapterFetchError::LoginRequired);
    }

    validate_chapter_capture(&engine.template.name, chapter, &cap, chapter_gate)?;

    let content = engine_novel::extract_chapter_from_html(engine, input_url, chapter, &cap.html)
        .map_err(|e| {
            ChapterFetchError::Failed(format!(
                "第 {} 章解析失敗（{} 段）: {}",
                chapter.index, cap.paragraph_count, e
            ))
        })?;

    let char_count = content.content.chars().count();
    if char_count == 0 {
        if webview_fetch::capture_is_gated(&cap, chapter_gate) {
            let _ = storage.remove_chapter(novel_dir, chapter);
            return Err(ChapterFetchError::LoginRequired);
        }
        return Err(ChapterFetchError::Failed(format!(
            "第 {} 章正文為空，可能未載入",
            chapter.index
        )));
    }

    storage
        .save_chapter(novel_dir, chapter, &content)
        .map_err(|e| ChapterFetchError::Failed(e.to_string()))?;
    Ok(char_count)
}

fn validate_chapter_capture(
    template_name: &str,
    chapter: &Chapter,
    cap: &CaptureResult,
    chapter_gate: Option<&GateRule>,
) -> Result<(), ChapterFetchError> {
    if webview_fetch::capture_is_gated(cap, chapter_gate) {
        return Err(ChapterFetchError::LoginRequired);
    }
    if cap.wait_status == "gate" {
        return Err(ChapterFetchError::LoginRequired);
    }
    if cap.wait_status == "timeout" {
        return Err(ChapterFetchError::Failed(format!(
            "第 {} 章擷取逾時（{} 字，wait={}）。請在抓取視窗確認正文已顯示後重試",
            chapter.index, cap.content_text_length, cap.wait_status
        )));
    }
    eprintln!(
        "[{}] chapter {} ok: {} blocks, {} text chars, {} html chars",
        template_name,
        chapter.index,
        cap.paragraph_count,
        cap.content_text_length,
        cap.html.len()
    );
    Ok(())
}

fn emit_chapter(
    app: &AppHandle,
    chapter: &Chapter,
    total: usize,
    status: ChapterStatus,
    char_count: usize,
) {
    emit(
        app,
        ProgressEvent::Chapter {
            chapter: ChapterProgress {
                index: chapter.index,
                total,
                title: chapter.title.clone(),
                id: chapter.id.clone(),
                char_count,
                status,
            },
        },
    );
}

fn emit_err(app: &AppHandle, msg: String) -> String {
    emit(app, ProgressEvent::Error { message: msg.clone() });
    msg
}
