// 透過 Tauri WebView 抓取頁面（支援手動通過 Cloudflare）

use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::oneshot;

use crate::scraper::engine::gate;
use crate::templates::schema::GateRule;

pub const FETCHER_LABEL: &str = "fetcher";

const WAIT_TIMEOUT_MS: u64 = 45_000;
const POLL_INTERVAL_MS: u64 = 400;
const POST_NAVIGATE_SETTLE_MS: u64 = 600;
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WaitMode {
    Index,
    Chapter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureResult {
    pub html: String,
    pub paragraph_count: usize,
    pub wait_status: String,
    #[serde(default)]
    pub content_text_length: usize,
    /// Live DOM matched `gates.chapter.selector` (only set when template provides one).
    #[serde(default)]
    pub gate_live: bool,
}

/// Whether capture should be skipped per template `gates.chapter` (no hard-coded selectors).
pub fn capture_is_gated(cap: &CaptureResult, chapter_gate: Option<&GateRule>) -> bool {
    if cap.gate_live || cap.wait_status == "gate" {
        return true;
    }
    chapter_gate
        .map(|r| gate::gate_matches(&cap.html, r))
        .unwrap_or(false)
}

/// Inject or clear `gates.chapter.selector` for live DOM checks in the fetcher.
pub fn set_chapter_gate_selector(app: &AppHandle, rule: Option<&GateRule>) -> Result<(), String> {
    let window = app
        .get_webview_window(FETCHER_LABEL)
        .ok_or_else(|| "抓取視窗未開啟".to_string())?;
    let js = match rule.and_then(|r| r.selector.as_deref()) {
        Some(sel) => format!(
            "window.__chapterGateSelector = {};",
            serde_json::to_string(sel).map_err(|e| e.to_string())?
        ),
        None => "window.__chapterGateSelector = null;".into(),
    };
    window.eval(&js).map_err(|e| e.to_string())
}

#[derive(Default)]
pub struct PendingCapture(pub Mutex<Option<oneshot::Sender<CaptureResult>>>);

#[derive(Default)]
pub struct PendingProbe(pub Mutex<Option<oneshot::Sender<String>>>);

/// 僅 hook Shadow DOM。不 hook fetch/XHR，避免干擾 Cloudflare Turnstile。
const SHADOW_HOOK_SCRIPT: &str = r#"
(function () {
    if (window.__shadowHookInstalled) return;
    window.__shadowHookInstalled = true;
    try {
        var origAttach = Element.prototype.attachShadow;
        window.__shadowHosts = [];
        Element.prototype.attachShadow = function (init) {
            var newInit = Object.assign({}, init || {}, { mode: 'open' });
            var sr = origAttach.call(this, newInit);
            try { window.__shadowHosts.push(this); } catch (e) {}
            return sr;
        };
    } catch (e) {}
})();
"#;

fn capture_script(mode: WaitMode) -> String {
    let mode_str = match mode {
        WaitMode::Index => "index",
        WaitMode::Chapter => "chapter",
    };
    format!(
        r#"
        (function () {{
            var invoke = window.__TAURI_INTERNALS__
                && window.__TAURI_INTERNALS__.invoke;
            if (!invoke) return;
            var mode = "{mode_str}";
            var deadline = Date.now() + {timeout};
            var interval = {interval};

            function findShadowContent() {{
                var hosts = (window.__shadowHosts || []).slice();
                document.querySelectorAll('*').forEach(function (el) {{
                    if (el.shadowRoot && hosts.indexOf(el) === -1) hosts.push(el);
                }});
                var best = null;
                for (var i = 0; i < hosts.length; i++) {{
                    var sr = hosts[i].shadowRoot;
                    if (!sr) continue;
                    var txt = (sr.textContent || '').trim();
                    if (!best || txt.length > best.textLen) {{
                        best = {{ root: sr, textLen: txt.length, html: sr.innerHTML }};
                    }}
                }}
                return best;
            }}

            function contentBody() {{
                var viewer = document.querySelector('article.novel-viewer');
                if (!viewer) return null;
                return viewer.querySelector('div[style*="--novel-font-size"]') || viewer;
            }}

            function contentTextLen() {{
                var sc = findShadowContent();
                if (sc && sc.textLen > 0) return sc.textLen;
                var b = contentBody();
                return b ? (b.innerText || '').trim().length : 0;
            }}

            function blockCount() {{
                var sc = findShadowContent();
                if (sc && sc.root) {{
                    var sps = sc.root.querySelectorAll('p');
                    if (sps.length) return sps.length;
                }}
                var b = contentBody();
                if (!b) return 0;
                var ps = b.querySelectorAll('p');
                if (ps.length) return ps.length;
                var direct = b.querySelectorAll(':scope > div');
                var n = 0;
                direct.forEach(function (d) {{
                    if ((d.innerText || '').trim()) n++;
                }});
                if (n) return n;
                var t = (b.innerText || '').trim();
                return t ? t.split(/\n+/).filter(function (l) {{ return l.trim(); }}).length : 0;
            }}

            function hasLiveGate() {{
                var sel = window.__chapterGateSelector;
                if (!sel) return false;
                try {{ return !!document.querySelector(sel); }}
                catch (e) {{ return false; }}
            }}

            function loadingVisible() {{
                var el = document.querySelector('.novel-loading');
                if (!el) return false;
                var t = (el.textContent || '').trim();
                if (!t || t.indexOf('불러') < 0) return false;
                var st = window.getComputedStyle(el);
                return st.display !== 'none' && st.visibility !== 'hidden' && el.offsetParent !== null;
            }}

            function indexReady() {{
                var root = document.querySelector(
                    'section.novel-detail, ul.novel-eps, .ep-list-v2'
                );
                if (!root) return false;
                var links = document.querySelectorAll(
                    'ul.novel-eps > li a[href], .ep-list-v2 li a[href]'
                );
                return links.length > 0;
            }}

            function ready() {{
                if (mode === 'chapter') {{
                    if (loadingVisible()) return false;
                    if (document.readyState !== 'complete') return false;
                    if (hasLiveGate()) return false;
                    return contentTextLen() > 0;
                }}
                if (mode === 'index') return indexReady();
                return document.readyState === 'complete';
            }}

            function buildHtml() {{
                var sc = findShadowContent();
                if (!sc || !sc.html || sc.textLen < 1) {{
                    return document.documentElement.outerHTML;
                }}
                var clone = document.documentElement.cloneNode(true);
                var article = clone.querySelector('article.novel-viewer');
                if (article) {{
                    article.innerHTML = sc.html;
                }} else {{
                    var body = clone.querySelector('body');
                    if (body) {{
                        var wrap = document.createElement('article');
                        wrap.className = 'novel-viewer';
                        wrap.innerHTML = sc.html;
                        body.appendChild(wrap);
                    }}
                }}
                return clone.outerHTML;
            }}

            function finish(status) {{
                invoke('submit_captured_html', {{
                    html: buildHtml(),
                    paragraphCount: blockCount(),
                    waitStatus: status,
                    contentTextLength: contentTextLen(),
                    gateLive: hasLiveGate()
                }});
            }}

            function tick() {{
                if (mode === 'chapter' && hasLiveGate()) return finish('gate');
                if (ready()) return finish('ready');
                if (Date.now() > deadline) return finish('timeout');
                setTimeout(tick, interval);
            }}

            if (document.readyState === 'loading') {{
                document.addEventListener('DOMContentLoaded', function () {{
                    setTimeout(tick, 300);
                }});
            }} else {{
                setTimeout(tick, 300);
            }}
        }})();
        "#,
        mode_str = mode_str,
        timeout = WAIT_TIMEOUT_MS,
        interval = POLL_INTERVAL_MS,
    )
}

async fn run_capture(app: &AppHandle, mode: WaitMode) -> Result<CaptureResult, String> {
    let window = app
        .get_webview_window(FETCHER_LABEL)
        .ok_or_else(|| "抓取視窗未開啟".to_string())?;

    let (tx, rx) = oneshot::channel::<CaptureResult>();
    {
        let state = app.state::<PendingCapture>();
        let mut guard = state.0.lock().map_err(|e| e.to_string())?;
        *guard = Some(tx);
    }

    window
        .eval(&capture_script(mode))
        .map_err(|e| format!("eval 失敗: {}", e))?;

    let result = tokio::time::timeout(Duration::from_secs(60), rx)
        .await
        .map_err(|_| "擷取 HTML 超時（60s）".to_string())?
        .map_err(|e| format!("接收 HTML 失敗: {}", e))?;

    if result.html.starts_with("__CAPTURE_ERROR__") {
        return Err(result.html);
    }

    eprintln!(
        "[capture] mode={:?} status={} blocks={} text_len={} html_len={}",
        mode,
        result.wait_status,
        result.paragraph_count,
        result.content_text_length,
        result.html.len()
    );

    Ok(result)
}


fn focus_fetcher(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(FETCHER_LABEL)
        .ok_or_else(|| "抓取視窗未開啟".to_string())?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

fn navigate_fetcher(app: &AppHandle, url: &str) -> Result<(), String> {
    let parsed: tauri::Url = url
        .parse()
        .map_err(|e: url::ParseError| e.to_string())?;
    let window = app
        .get_webview_window(FETCHER_LABEL)
        .ok_or_else(|| "抓取視窗未開啟".to_string())?;
    // 章節批次下載時只換網址，不 show/set_focus，避免每次導航搶走前景
    window.navigate(parsed).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn open_fetch_window(app: AppHandle, url: String) -> Result<(), String> {
    let parsed: tauri::Url = url.parse().map_err(|e: url::ParseError| e.to_string())?;

    if app.get_webview_window(FETCHER_LABEL).is_some() {
        return focus_fetcher(&app);
    }

    // CF 階段不注入任何 initialization_script，避免 Turnstile 被干擾
    WebviewWindowBuilder::new(&app, FETCHER_LABEL, WebviewUrl::External(parsed))
        .title("抓取頁面 - 請手動完成 Cloudflare 驗證")
        .inner_size(1100.0, 800.0)
        .resizable(true)
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// 目錄抓完後重建抓取視窗並注入 Shadow hook（cookie 保留，通常不需再過 CF）
pub async fn recreate_fetcher_with_hook(app: AppHandle, url: String) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(FETCHER_LABEL) {
        window.close().map_err(|e| e.to_string())?;
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    let parsed: tauri::Url = url.parse().map_err(|e: url::ParseError| e.to_string())?;

    WebviewWindowBuilder::new(&app, FETCHER_LABEL, WebviewUrl::External(parsed))
        .title("抓取頁面")
        .inner_size(1100.0, 800.0)
        .resizable(true)
        .focused(false)
        .initialization_script(SHADOW_HOOK_SCRIPT)
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
}

pub async fn fetch_page(
    app: &AppHandle,
    url: &str,
    mode: WaitMode,
    navigate: bool,
) -> Result<CaptureResult, String> {
    if matches!(mode, WaitMode::Index) {
        if app.get_webview_window(FETCHER_LABEL).is_none() {
            return Err("請先按「開啟抓取視窗」並完成 Cloudflare 驗證".into());
        }
        focus_fetcher(app)?;
        return run_capture(app, mode).await;
    }

    if app.get_webview_window(FETCHER_LABEL).is_none() {
        open_fetch_window(app.clone(), url.to_string()).await?;
    } else if navigate {
        navigate_fetcher(app, url)?;
    } else {
        focus_fetcher(app)?;
    }

    tokio::time::sleep(Duration::from_millis(POST_NAVIGATE_SETTLE_MS)).await;
    run_capture(app, mode).await
}

/// 站點常封鎖 F12；由 App 注入腳本探測 DOM，不依賴 DevTools。
const PROBE_PAGE_SCRIPT: &str = r#"
(function () {
    var invoke = window.__TAURI_INTERNALS__
        && window.__TAURI_INTERNALS__.invoke;
    if (!invoke) return;

    function shadowStats() {
        var hosts = (window.__shadowHosts || []).slice();
        document.querySelectorAll('*').forEach(function (el) {
            if (el.shadowRoot && hosts.indexOf(el) === -1) hosts.push(el);
        });
        var best = { textLen: 0, pCount: 0, hostTag: '', hostClass: '' };
        for (var i = 0; i < hosts.length; i++) {
            var sr = hosts[i].shadowRoot;
            if (!sr) continue;
            var txt = (sr.textContent || '').trim();
            var ps = sr.querySelectorAll('p').length;
            if (txt.length > best.textLen) {
                best = {
                    textLen: txt.length,
                    pCount: ps,
                    hostTag: hosts[i].tagName,
                    hostClass: hosts[i].className || ''
                };
            }
        }
        return { hostCount: hosts.length, best: best };
    }

    function epRows() {
        var rows = document.querySelectorAll('ul.novel-eps > li, .ep-list-v2 li');
        return Array.prototype.slice.call(rows, 0, 80).map(function (li) {
            var a = li.querySelector('a');
            return {
                classes: li.className,
                text: (li.innerText || '').trim().slice(0, 120),
                href: a ? (a.href || a.getAttribute('href') || '') : ''
            };
        });
    }

    var viewer = document.querySelector('article.novel-viewer');
    var viewerText = viewer ? (viewer.innerText || '').trim() : '';
    var loginRe = /로그인|회원|로그인이|로그인 후|로그인하|sign.?in|log.?in/i;
    var loginLinks = [];
    document.querySelectorAll('a[href]').forEach(function (a) {
        var h = (a.getAttribute('href') || '').toLowerCase();
        if (h.indexOf('login') >= 0 || h.indexOf('sign') >= 0 || h.indexOf('auth') >= 0) {
            loginLinks.push({
                href: a.href,
                text: (a.innerText || '').trim().slice(0, 80)
            });
        }
    });

    var loadingEl = document.querySelector('.novel-loading');
    var payload = {
        url: location.href,
        title: document.title,
        pageKind: document.querySelector('ul.novel-eps, .ep-list-v2')
            ? 'index'
            : (viewer ? 'chapter' : 'other'),
        shadow: shadowStats(),
        viewerTextPreview: viewerText.slice(0, 600),
        viewerHtmlPreview: viewer ? viewer.innerHTML.slice(0, 2000) : null,
        loginHints: {
            viewerHasLoginWord: loginRe.test(viewerText),
            bodySnippetHasLoginWord: loginRe.test(
                (document.body && document.body.innerText || '').slice(0, 8000)
            ),
            loginLinks: loginLinks.slice(0, 25)
        },
        epRowsSample: epRows(),
        novelLoadingText: loadingEl ? (loadingEl.textContent || '').trim() : null
    };

    invoke('submit_probe_result', { json: JSON.stringify(payload, null, 2) });
})();
"#;

async fn run_probe(app: &AppHandle) -> Result<String, String> {
    let window = app
        .get_webview_window(FETCHER_LABEL)
        .ok_or_else(|| "請先按「開啟抓取視窗」".to_string())?;

    let (tx, rx) = oneshot::channel::<String>();
    {
        let state = app.state::<PendingProbe>();
        let mut guard = state.0.lock().map_err(|e| e.to_string())?;
        *guard = Some(tx);
    }

    window
        .eval(PROBE_PAGE_SCRIPT)
        .map_err(|e| format!("探測腳本 eval 失敗: {}", e))?;

    tokio::time::timeout(Duration::from_secs(15), rx)
        .await
        .map_err(|_| "探測逾時（15s）：請確認抓取視窗已載入目標頁".to_string())?
        .map_err(|e| format!("接收探測結果失敗: {}", e))
}

fn write_probe_file(save_dir: Option<&str>, json: &str) -> Result<String, String> {
    let dir = match save_dir {
        Some(d) if !d.trim().is_empty() => std::path::PathBuf::from(d),
        _ => std::env::temp_dir(),
    };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = dir.join(format!("scraper-probe-{}.json", ts));
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn probe_fetcher_page(
    app: AppHandle,
    save_dir: Option<String>,
) -> Result<String, String> {
    focus_fetcher(&app)?;
    let json = run_probe(&app).await?;
    let path = write_probe_file(save_dir.as_deref(), &json)?;
    eprintln!("[probe] saved to {}", path);
    Ok(path)
}

#[tauri::command]
pub fn submit_probe_result(app: AppHandle, json: String) -> Result<(), String> {
    let state = app.state::<PendingProbe>();
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(tx) = guard.take() {
        let _ = tx.send(json);
    }
    Ok(())
}

#[tauri::command]
pub fn submit_captured_html(
    app: AppHandle,
    html: String,
    paragraph_count: Option<usize>,
    wait_status: Option<String>,
    content_text_length: Option<usize>,
    gate_live: Option<bool>,
) -> Result<(), String> {
    let result = CaptureResult {
        html,
        paragraph_count: paragraph_count.unwrap_or(0),
        wait_status: wait_status.unwrap_or_else(|| "unknown".into()),
        content_text_length: content_text_length.unwrap_or(0),
        gate_live: gate_live.unwrap_or(false),
    };
    let state = app.state::<PendingCapture>();
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(tx) = guard.take() {
        let _ = tx.send(result);
    }
    Ok(())
}
