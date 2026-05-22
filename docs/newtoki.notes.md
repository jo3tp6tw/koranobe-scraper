# newtoki / sbxh1 反爬與破解筆記

> 對象站點：뉴토끼（newtoki）系列韓國盜版小說/漫畫站
> 範例網域：`sbxh1.com`（同集團網域會週期性輪換：sbxh1 / sbxh2 / sktoki / moontoki ...）
> 範例章節：`https://sbxh1.com/novel/57467/5146780`

## 一、網站使用的反爬技術棧

依**伺服端 → 客戶端**順序疊上去，每層都會卡住常規爬蟲：

### 1. Cloudflare（人機驗證 + 直連阻擋）
- 用 `reqwest` 直接抓 JS chunk → 直接 `403 Forbidden`。
- 章節初次造訪可能要過 Cloudflare 挑戰頁。
- 必須用真實瀏覽器 / WebView 跑完 JS 才能拿到正常 200。

### 2. Next.js App Router + RSC 串流（內容不在 SSR HTML 裡）
- 初始 HTML 只有殼，正文「不存在」於 `document.documentElement.outerHTML`。
- RSC payload 透過 `self.__next_f.push([...])` 一塊塊塞入 `<script>` 標籤，但其中**也只有 metadata**（小說標題、`novelId`、`episodeId`、`token`、`cookieName:"nv"` 等），**沒有章節正文**。
- 也就是說：拿到完整 HTML 你還是看不到內文。

### 3. 章節內容用 API 動態載入
- `<NovelContent>` 客戶端元件啟動後會打：
  - `POST /api/nv-issue`（先取 issue / session）
  - `POST /api/novel-content`（取章節 payload）
- 回傳長這樣（**已混淆**，不是純文字）：
  ```json
  { "ok": true, "payload": "gEGx3VEWK_VR8MDxDsciuzSjrO5m-_AbFHHk4JN8zN63jmLLL5jIRVEAbTYZfxA7..." }
  ```
- 直接抓這個 payload 也沒用，是 base64-url 樣式的加密字串。

### 4. WebAssembly 字串解擾（unshuffle）
- 頁面會載入 `/_next/static/media/novel_unshuffle_bg.wasm`。
- 其中 export 一個 `unshuffle(payload, ...) -> string` 函式（從匯出符號 `__wbg___wbindgen_throw_*` 看得出來是 wasm-bindgen 編出的 Rust）。
- JS 把 API 拿到的 payload 餵進 WASM 解出真正的章節 HTML（含 `<p>` 標籤）。

### 5. **Closed Shadow DOM 隔離**（壓死駱駝的最後一根稻草）
- 解碼出來的 HTML **不會** 直接 `innerHTML =` 到頁面元素裡。
- JS 對 `<div style="--novel-font-size:16px">` 呼叫：
  ```js
  div.attachShadow({ mode: 'closed' });
  ```
  然後把解碼後 HTML 塞進這個 closed shadow root。
- Closed shadow root 的特性：
  - 外部 JS 讀 `host.shadowRoot` → `null`
  - `outerHTML` / `innerHTML` 抓不到內容
  - `document.querySelectorAll('p')` 看不到裡面的 `<p>`
  - 但**視覺上正常顯示**、**`getSelection()` 雙擊單字可以選到字**（這是判斷它存在的關鍵線索）。

### 6. 視覺干擾（CSS 阻止橫向選取）
- 加上 `user-select` 限制、`pointer-events` 之類的 CSS，讓使用者拖曳很難一次選到整段——但雙擊單字仍可複製。這個基本是用來騙人類，跟程式抓取無關。

---

## 二、組合起來為什麼難抓

| 嘗試 | 結果 |
|------|------|
| `reqwest` 抓 HTML | 401/403 或拿到空殼 |
| 解析 `__next_f` RSC 串流 | 只有 metadata，無正文 |
| WebView 載入後讀 `documentElement.outerHTML` | 124K 字元的空殼 |
| WebView 載入後讀 `article.novel-viewer.innerHTML` | toolbar + 空 `<div>` |
| WebView 跑 `document.querySelectorAll('body *')` 掃韓文 | 只找到留言、UI 文字、`<script>` 內 RSC |
| WebView 用 `getSelection()` 全選 article | 14 字（toolbar 文字而已） |
| 找 iframe / canvas | 0 個 |
| 找 `::before` / `::after` content | 空 |
| 找 `data-*` 屬性 | 空 |

→ **每一種常規方法都失敗。**

但 `article.scrollHeight = 4821px`（明明佔了空間）、使用者雙擊可以複製到字、`anchorNode.parentElement === article.novel-viewer`（卻 innerHTML 是空）——三條矛盾線索拼起來只有一個解釋：**closed Shadow DOM**。

---

## 三、破解方法（目前實作）

**核心 trick**：在頁面任何 JS 跑之前，monkey-patch `Element.prototype.attachShadow`，把所有 `mode: 'closed'` 強制改成 `mode: 'open'`，並把 host 元素記錄起來。

### 實作位置
- Tauri WebView 用 `WebviewWindowBuilder::initialization_script(...)` 注入 `NET_HOOK_SCRIPT`，這個腳本會在每次 navigation 的 page-script 之前執行。
- 程式檔：`src-tauri/src/webview_fetch.rs`

### Hook 程式碼（簡化版）
```js
var origAttach = Element.prototype.attachShadow;
window.__shadowHosts = [];
Element.prototype.attachShadow = function (init) {
    var newInit = Object.assign({}, init || {}, { mode: 'open' });
    var sr = origAttach.call(this, newInit);
    window.__shadowHosts.push(this);
    return sr;
};
```

被 hook 後，原本「closed」的 shadow root 全部變「open」，外部 JS 就能用 `host.shadowRoot` 拿到。

### 擷取階段
等待頁面把章節 inject 完（用 shadow root 的 `textContent.length >= 800` 判定 ready），然後：

```js
var hosts = (window.__shadowHosts || []).slice();
document.querySelectorAll('*').forEach(el => {
    if (el.shadowRoot && hosts.indexOf(el) === -1) hosts.push(el);
});
// 找 textContent 最長的 shadow root（就是正文）
var best = ...;
// 把它的 innerHTML 灌回 article.novel-viewer 給後續 CSS selector 解析
article.innerHTML = best.html;
```

### Template 設定（newtoki.json）
```json
"content": {
  "type": "css",
  "selector": "article.novel-viewer",
  "extract": "html",
  "post": [{ "op": "select_all", "selector": "p", "join": "\n\n" }]
}
```

`<p>` 段落直接從注入回 article 的 HTML 中抓出，每段以雙換行接起來。

---

## 四、付費會차 / 需登入（探測 2026-05）

規則寫在 **`src-tauri/templates/newtoki.json`** 的 `gates` 與 `outputs.paid_gate`（見 `json.md` §9.1），不要改 Rust 硬編碼。

| 頁面 | 模板設定 |
|------|----------|
| 章節頁 | `gates.chapter.selector`: `article.novel-viewer .novel-paid-gate` |
| 目錄頁 | `gates.index_item.html_contains` + `chapters.item.paid_gate` css |

App 會自動跳過（狀態 `loginRequired`），不實作登入。

---

## 五、注意事項與維運提醒

1. **必須完全關掉抓取視窗再重開**——`initialization_script` 只在新建 WebView 視窗時生效；reload 頁面不會重新注入。
2. **網域漂移**：`sbxh1.com` 隨時可能換成 `sbxh2` / 別的數字。可以做一個小工具去 newtoki 官方 Telegram 頻道 / Google 取現行網域，或在 template 動態替換。
3. **Cloudflare 二次驗證**：連抓上百章可能會跳 CAPTCHA。建議：
   - `request_delay_ms` ≥ 1200，加 ±300ms 隨機 jitter
   - 偵測抓到的 HTML 是 Cloudflare 挑戰頁就暫停讓使用者過驗證
4. **斷點續抓**：以 chapter `href` 中的 `episodeId` 為 key 記錄已完成清單。
5. **網站若日後升級**（例如把 `attachShadow` 也 hook 起來檢查、或改用 native 自繪）：
   - 第一手線索：使用者能視覺看到、`getSelection()` 也讀得到 → 改用 Selection API 全選 + 走 Selection.toString()。
   - 終極手段：在 Rust 端載入 `novel_unshuffle_bg.wasm`（用 `wasmtime` / `wasmer`）自己呼叫 `unshuffle(payload)`，繞過整個 DOM 渲染流程。

---

## 六、診斷工具（已內建於 App）

開發/排查時 UI 提供：
- **3b. 立即擷取**：dump 整頁 HTML（不等待）
- **3c. 傾印網路請求**：dump 所有 `fetch` / XHR（找出 `/api/novel-content` 之類的端點）
- **3d. 讀取當前選取**：使用者在 webview 雙擊任意文字後執行，回報該文字節點的 8 層父鏈 selector
- **4. 抽取章節正文**：用 Selection API + Shadow DOM 掃描回報正文位置與長度
- **2. 探測正文 DOM → 儲存報告**：傾印 viewer 結構、style/attr/pseudo/iframe/canvas、韓文文字節點分組

排查未知反爬手段時依序 3c → 4 → 2，三個一起用基本能找出絕大多數隱藏渲染方式。
