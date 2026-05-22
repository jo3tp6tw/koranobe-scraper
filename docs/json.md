# 小說爬蟲模板（Template JSON）撰寫指南

這個資料夾裡每個 `.json` 檔都是一份「網站爬蟲設定」。執行時 Rust 端的 `scraper::engine::Engine` 會讀進去、根據裡面的指令逐頁抓取，**完全不需要寫 Rust 程式碼就能新增一個新的小說站**。

對應的 Rust 型別定義在 `src/templates/schema.rs`，執行邏輯在 `src/scraper/engine/`。

---

## 1. 整體結構

```jsonc
{
  "name": "kakuyomu",          // 模板識別名稱（給 built_in::get 用）
  "version": 1,                 // 預留版本號，目前固定 1
  "base_url": "https://kakuyomu.jp",

  "fetch": {                    // 可省略；以下是預設值
    "request_delay_ms": 600,    // 連續抓取間的延遲
    "retry": { "attempts": 3, "backoff_ms": 800 },
    "concurrency": 1            // 目前固定循序
  },

  "vars": {                     // 從輸入 URL 算出來的變數（可省略）
    "work_id": { "from": "url", "type": "regex", "pattern": "/works/(\\d+)" }
  },

  "pages": {                    // 每個 page 是一個抓取流程
    "index":   { ... },
    "chapter": { ... }
  }
}
```

引擎本身對 `pages` 的命名沒有限制，但**小說 adapter 約定有兩個 page**：
- `index`：抓目錄頁，必須輸出 `title` / `introduction` / `tags` / `chapters`
- `chapter`：抓單章，必須輸出 `title` / `content`

詳細欄位約定見最後一節「Novel Adapter 約定」。

---

## 2. `vars` — 從 URL 萃取變數

目前唯一支援的來源是 `url` + 正規表示式。例如要從 `https://kakuyomu.jp/works/16817330663095755643` 取出作品 ID：

```json
"vars": {
  "work_id": {
    "from": "url",
    "type": "regex",
    "pattern": "/works/(\\d+)",
    "group": 1
  }
}
```

之後在任何模板字串裡都能用 `{vars.work_id}` 引用。

也可以宣告常數變數：

```json
"site_id": { "from": "const", "value": "kakuyomu" }
```

---

## 3. `pages.<name>` — 一個頁面流程

```jsonc
{
  "url": "{input}",                            // URL 模板（見「佔位符」一節）
  "iterate": "pages.index.outputs.chapters",   // 可省略；若有則該 page 會被 fan-out
  "source": { "type": "html" },                // 資料來源
  "outputs": {                                  // 要抽什麼資料出來
    "title":   { "type": "css", "selector": "h1" },
    "content": { "type": "css", "selector": ".body" }
  }
}
```

> `iterate` 目前只是給 adapter 看的提示文字，引擎會由 adapter（`engine::novel`）根據 index 的 `chapters` 陣列做 fan-out，逐筆建立 `{item.x}` 後重跑這個 page。

---

## 4. `source` — 資料來源型態

### 4.1 `html`（純網頁）

```json
"source": { "type": "html" }
```

CSS extractor 會作用在抓回來的 HTML 上。

#### 翻頁（paginate）

加上 `paginate` 子物件可以自動把多頁清單合併：

```json
"source": {
  "type": "html",
  "paginate": {
    "strategy": "next_link",
    "selector": "a.c-pager__item--next",   // 「下一頁」連結
    "attr": "href",
    "resolve": "absolute",                  // 把相對網址補成絕對
    "merge": ["chapters"],                  // 哪些 outputs 要跨頁串接
    "max_pages": 100                        // 安全上限
  }
}
```

只有列在 `merge` 的 output（必須是陣列）會跨頁累積；其他欄位（如 title）只取第一頁。
若選不到下一頁連結就停止；造訪過的 URL 會被去重避免無限迴圈。

### 4.2 `next_data`（Next.js 站點）

```json
"source": {
  "type": "next_data",
  "deref_refs": true,
  "root": "$.props.pageProps.__APOLLO_STATE__['Work:{vars.work_id}']"
}
```

引擎會：
1. 從頁面找到 `<script id="__NEXT_DATA__">…</script>` 並 parse 成 JSON
2. 若 `deref_refs: true`：遞迴把 `{ "__ref": "Foo:1" }` 物件展開成 `__APOLLO_STATE__["Foo:1"]` 的真實內容
3. 若有 `root`：先做模板插值，再用 mini-JSONPath 把 JSON 根節點移到指定處

之後 `jsonpath` extractor 就以這個根節點為起點。
HTML 也仍保留著，所以在同一個 page 也可以混用 `css` extractor。

---

## 5. `outputs.<name>` — Extractor

每個 output 都是一個 extractor。`type` 決定形狀：

### 5.1 `css`

```json
{
  "type": "css",
  "selector": "h1.title",
  "extract": "text",        // text | html | attr，預設 text
  "attr": "href",            // 當 extract = "attr" 時必填
  "resolve": "absolute",     // 對結果（URL）做絕對化；可省略
  "multiple": false,         // 是否把所有匹配結果回傳成陣列
  "post": [ ... ]            // 後處理（見下節）
}
```

`text` 模式會把元素內所有文字串接並把連續空白壓成單一空格。

### 5.2 `jsonpath`（只能用在 `next_data` 來源）

```json
{ "type": "jsonpath", "path": "$.title" }
{ "type": "jsonpath", "path": "$.tagLabels[*]", "multiple": true }
```

支援的迷你 JSONPath 語法（**不是完整 JSONPath**）：

| 語法 | 說明 |
| --- | --- |
| `$` | 根節點 |
| `.field` | 物件欄位 |
| `['weird:key']` | 含特殊字元的欄位（單／雙引號皆可） |
| `[*]` | 陣列展開（也可展開物件成 values） |

`multiple: false` 時取第一個匹配；找不到回傳 `null`。
`multiple: true` 時所有匹配收成陣列。

### 5.3 `list`

最重要的工具：把容器內每個 item 變成一個物件陣列。**HTML 模式**用 CSS selector：

```json
"chapters": {
  "type": "list",
  "selector": ".p-eplist__sublist",
  "item": {
    "title": { "type": "css", "selector": "a.p-eplist__subtitle" },
    "url":   { "type": "css", "selector": "a.p-eplist__subtitle",
               "extract": "attr", "attr": "href", "resolve": "absolute" }
  }
}
```

**JSON 模式**用 path：

```json
"chapters": {
  "type": "list",
  "path": "$.tableOfContentsV2[*].episodeUnions[*]",
  "item": {
    "id":    { "type": "jsonpath", "path": "$.id" },
    "title": { "type": "jsonpath", "path": "$.title" },
    "url":   { "type": "template",
               "value": "{base_url}/works/{vars.work_id}/episodes/{item.id}" }
  }
}
```

`item` 內的子 extractor 可以透過 `{item.x}` 讀到：
- JSON 模式：當前 item 的原始 JSON 欄位
- HTML 模式：到目前為止已算好的兄弟欄位

也就是說，`item` 內的欄位順序有意義 — 後面的可以引用前面的。

### 5.4 `template`

把模板字串組合成字串。常用來拼章節 URL：

```json
{ "type": "template",
  "value": "{base_url}/works/{vars.work_id}/episodes/{item.id}",
  "resolve": "absolute" }
```

### 5.5 `const`

固定值。

```json
{ "type": "const", "value": "Kakuyomu" }
{ "type": "const", "value": ["a", "b"] }
```

---

## 6. `post` — 字串後處理鏈

只能加在 `css` extractor 上，逐步處理萃取出來的字串。

### `select_all`

把當前字串當 HTML，跑一次 selector，把每個元素的文字 join 起來。
**最常用：把 `<div class="body">` 抓成 html，再用 `select_all` 把 `<p>` 一段段串成換行分隔的純文字。**

```json
{
  "type": "css",
  "selector": "div.widget-episodeBody",
  "extract": "html",
  "post": [
    { "op": "select_all", "selector": "p", "join": "\n" }
  ]
}
```

### `trim`

```json
{ "op": "trim" }
```

---

## 7. 模板字串佔位符

下列佔位符可以用在任何 `url`、`template.value`、`source.root` 字串裡：

| 佔位符 | 含義 |
| --- | --- |
| `{input}` | 使用者傳進來的原始 URL |
| `{base_url}` | template 頂層的 `base_url` |
| `{vars.<name>}` | `vars` 區段算出來的變數，支援 `vars.a.b` 鏈式 |
| `{item.<field>}` | 在 `list` 子 extractor 或 `iterate` page 裡的當前項目 |

**找不到欄位會直接報錯**，幫助你盡早發現模板拼錯。

---

## 8. URL 解析（`resolve` 的語意）

當 `resolve: "absolute"` 時：
- 已是 `http://` / `https://`：原樣保留
- 是路徑（`/foo/bar`）：拼到 base_url 的 host 上
- 是 query-only（`?p=2`）：附加到當前頁的 path 上
- 是相對路徑（`./bar`）：對 current_url 做標準 URL join

`resolve: "raw"`（或不寫）就完全不動。

---

## 9. Novel Adapter 約定

### 9.1 `gates` — 跳過需登入 / 付費會차（可省略）

newtoki 等站點可在模板頂層宣告 **gate 規則**，Rust 下載器會跳過匹配章節（不登入、不當失敗）：

```json
"gates": {
  "chapter": {
    "selector": "article.novel-viewer .novel-paid-gate",
    "html_contains": ["novel-paid-gate", "유료 회차"]
  },
  "index_item": {
    "html_contains": [">P<", "novel-paid", "ne-pt"]
  }
}
```

| 欄位 | 用途 |
| --- | --- |
| `gates.chapter` | 章節頁 HTML / WebView 擷取結果：有則跳過 |
| `gates.index_item` | 目錄頁中，含 `{item.id}` 的那列 `<li>` 片段 |

同一規則也可在 `pages.*.outputs` 用 **css extractor** 抽出（例如 `paid_gate`），adapter 會把非空的 `chapters.item.paid_gate` 記成 `Chapter.requires_login`。

---

## 9.2 Novel Adapter 欄位約定

`engine::novel::fetch_novel_info` 會跑 `pages.index` 並期望 outputs 含：

```jsonc
{
  "title":        "string",
  "introduction": "string",          // 可缺，缺則為空字串
  "tags":         ["string", ...],   // 可缺，缺則為空陣列
  "chapters": [
    { "title": "...", "url": "...", "id": "...", "requires_login": false },
    // requires_login 由 item.paid_gate 或 gates.index_item 推導；可省略
    ...
  ]
}
```

`engine::novel::fetch_chapter` 會跑 `pages.chapter`，並把當前 chapter（含 `id` / `title` / `url` / `index`）放入 `{item.*}`。期望 outputs 含：

```jsonc
{
  "title":   "string",   // 可缺，缺則為空字串
  "content": "string"
}
```

---

## 10. 完整範例

### 10.1 Kakuyomu（Next.js + Apollo refs）

見 `kakuyomu.json`。重點：
- 用 `vars.work_id` 從輸入 URL 抽出作品 ID
- `source.next_data` + `deref_refs` 把 `__APOLLO_STATE__` 的 `__ref` 物件全部解開，章節結構直接變成可遍歷的樹
- `chapters` 用 JSON list + template URL 組裝出可直接抓的章節 URL，不需要再點頁面拿 href

### 10.2 Syosetu（純 HTML + 翻頁）

見 `syosetu.json`。重點：
- 純 HTML，不用 next_data
- `paginate` 用 `a.c-pager__item--next` 自動翻頁，把每一頁的 `chapters` 串成完整目錄
- 章節標題的 `<a>` 已經帶相對 URL，用 `resolve: "absolute"` 直接補成絕對網址

---

## 11. 新增一個站點的最小流程

1. 開好兩個 fixture：把目標站的目錄頁 / 一個章節頁存成本地 HTML
2. 在 `templates/` 建 `<site>.json`，先把 `name` / `base_url` / `pages.index.outputs.title` 填好
3. 在 `built_in.rs::get` 增加新分支
4. 在 `scraper::engine::fixture_tests` 仿照 syosetu / kakuyomu 那兩個測試，把你的 fixture include 進來，逐步把 outputs 加完直到測試 pass
5. 不需要動任何 Rust 主程式 — `commands::download_novel` 會直接認識新模板

---

## 12. 限制與已知粗糙之處

- 迷你 JSONPath 不支援 filter（`?(@.x == 1)`）、不支援 recursive descent（`..`）、不支援切片
- `paginate` 目前只有 `next_link` 一種策略；未來可加 `query_param` / `numbered`
- `concurrency` 欄位保留但目前固定循序執行；rate limit 由 `request_delay_ms` 控制
- 沒有 cookie / 動態 JS 支援；遇到 Cloudflare / 需要瀏覽器渲染的站點需另外的 fetcher（見 `scraper::browser`）
