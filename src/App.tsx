import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { NavBar } from "./components/NavBar/NavBar";
import { DebugPage } from "./pages/DebugPage";
import { HelpPage } from "./pages/HelpPage";
import { HomePage } from "./pages/HomePage";
import { SettingsPage } from "./pages/SettingsPage";
import type { AppPage } from "./types/navigation";
import type {
  ChapterListItem,
  ListNovelChaptersResponse,
  ProgressEvent,
} from "./types/novel";
import { loadSettings, saveSettings } from "./lib/settings";
import "./App.css";

const DEFAULT_CHAPTER_DELAY_MS = 1200;
const savedSettings = loadSettings();

function App() {
  const [page, setPage] = useState<AppPage>("home");
  const [novelUrl, setNovelUrl] = useState(savedSettings.novelUrl ?? "");
  const [workingFolder, setWorkingFolder] = useState(savedSettings.workingFolder ?? "");
  const [progress, setProgress] = useState<string[]>([]);
  const [fetcherOpen, setFetcherOpen] = useState(false);
  const [isDownloading, setIsDownloading] = useState(false);
  const [isListing, setIsListing] = useState(false);
  const [done, setDone] = useState(false);
  const [chapterList, setChapterList] = useState<ChapterListItem[]>([]);
  const [novelTitle, setNovelTitle] = useState("");
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [forceOverwrite, setForceOverwrite] = useState(savedSettings.forceOverwrite ?? false);
  const [chapterDelayMs, setChapterDelayMs] = useState(
    savedSettings.chapterDelayMs ?? DEFAULT_CHAPTER_DELAY_MS
  );

  useEffect(() => {
    saveSettings({ novelUrl, workingFolder, forceOverwrite, chapterDelayMs });
  }, [novelUrl, workingFolder, forceOverwrite, chapterDelayMs]);

  useEffect(() => {
    const unlisten = listen<ProgressEvent>("scrape-progress", (event) => {
      const payload = event.payload;
      setProgress((prev) => [...prev, ...formatProgressLines(payload)]);
      if (payload.stage === "done") {
        setIsDownloading(false);
        setDone(true);
      } else if (payload.stage === "cancelled") {
        setIsDownloading(false);
      } else if (payload.stage === "error") {
        setIsDownloading(false);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  function skipReasonLabel(reason: string): string {
    if (reason === "loginRequired") return "付費/需登入";
    if (reason === "alreadyExists") return "已存在";
    return reason;
  }

  function formatChapterLine(
    index: number,
    title: string,
    id: string,
    charCount: number | null,
    suffix: string
  ): string {
    const num = String(index).padStart(4, "0");
    const chars = charCount === null ? "" : ` | ${charCount} 字`;
    return `${num} | ${title} | ${id}${chars} | ${suffix}`;
  }

  function formatProgressLines(payload: ProgressEvent): string[] {
    switch (payload.stage) {
      case "start":
        return [`開始下載: ${payload.url}`];
      case "fetchingIndex":
        return ["正在擷取目錄頁..."];
      case "waitingCloudflare":
        return [`請在抓取視窗完成 Cloudflare 驗證: ${payload.url}`];
      case "novelInfo":
        return [`標題: ${payload.info?.title}，章節數: ${payload.info?.chapters?.length}`];
      case "skipPreview": {
        const items = payload.items ?? [];
        if (items.length === 0) {
          return [];
        }
        const detail = items
          .map((it) =>
            formatChapterLine(it.index, it.title, it.id, null, skipReasonLabel(it.reason))
          )
          .join("；");
        return [`將跳過（${items.length}）：${detail}`];
      }
      case "chapter": {
        const ch = payload.chapter;
        if (!ch || ch.index == null) return [];
        const st = ch.status ?? "";
        const label =
          st === "loginRequired"
            ? "跳過（付費/需登入）"
            : st === "skipped"
              ? "已存在"
              : st === "downloaded"
                ? "已下載"
                : st === "failed"
                  ? "失敗"
                  : st;
        return [
          formatChapterLine(ch.index, ch.title, ch.id, ch.charCount, label),
        ];
      }
      case "done":
        return [`完成! 存放位置: ${payload.folder}`];
      case "cancelled":
        return [`已停止: ${payload.message ?? ""}（${payload.folder ?? ""}）`];
      case "error":
        return [`錯誤: ${payload.message}`];
      default:
        return [JSON.stringify(payload)];
    }
  }

  async function selectFolder() {
    const folder = await invoke<string | null>("select_working_folder");
    if (folder) setWorkingFolder(folder);
  }

  async function openFetcher() {
    if (!novelUrl) {
      alert("請先填入小說 URL");
      return;
    }
    try {
      await invoke("open_fetch_window", { url: novelUrl });
      setFetcherOpen(true);
      setProgress((prev) => [
        ...prev,
        "已開啟抓取視窗，請在該視窗完成 Cloudflare 驗證，確認目錄頁出現後按「② 列出章節」",
      ]);
    } catch (e) {
      alert(`開啟抓取視窗失敗: ${e}`);
    }
  }

  async function probeCurrentPage() {
    if (!fetcherOpen) {
      alert("請先按「開啟抓取視窗」並在該視窗打開要分析的頁面");
      return;
    }
    try {
      const path = await invoke<string>("probe_fetcher_page", {
        saveDir: workingFolder || null,
      });
      setProgress((prev) => [
        ...prev,
        `探測報告已寫入: ${path}（站點封 F12 時請用此功能）`,
      ]);
    } catch (e) {
      alert(`探測失敗: ${e}`);
    }
  }

  async function listChapters() {
    if (!novelUrl) {
      alert("請先填入小說 URL");
      return;
    }
    if (!fetcherOpen) {
      alert("請先按「開啟抓取視窗」並完成 Cloudflare 驗證");
      return;
    }
    setIsListing(true);
    try {
      const result = await invoke<ListNovelChaptersResponse>("list_novel_chapters", {
        url: novelUrl,
        workingFolder: workingFolder || null,
        templateName: "newtoki",
      });
      setNovelTitle(result.title);
      setChapterList(result.chapters);
      setChapterDelayMs(result.defaultChapterDelayMs);
      const defaultSelected = new Set(
        result.chapters
          .filter(
            (ch) =>
              !ch.requiresLogin &&
              (forceOverwrite || ch.localStatus !== "exists")
          )
          .map((ch) => ch.id)
      );
      setSelectedIds(defaultSelected);
      setProgress((prev) => [
        ...prev,
        `已列出章節：${result.title}（共 ${result.chapters.length} 章）`,
      ]);
    } catch (e) {
      alert(`列出章節失敗: ${e}`);
    } finally {
      setIsListing(false);
    }
  }

  function toggleChapter(id: string) {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }

  function selectAllChapters() {
    setSelectedIds(
      new Set(chapterList.filter((ch) => !ch.requiresLogin).map((ch) => ch.id))
    );
  }

  function deselectAllChapters() {
    setSelectedIds(new Set());
  }

  function handleNovelUrlChange(url: string) {
    setNovelUrl(url);
    setFetcherOpen(false);
    setChapterList([]);
    setSelectedIds(new Set());
  }

  async function downloadSelected() {
    if (!novelUrl || !workingFolder) {
      alert("請先填入小說 URL 並選擇存放目錄");
      return;
    }
    if (selectedIds.size === 0) {
      alert("請至少選擇一個章節");
      return;
    }
    setIsDownloading(true);
    setProgress([]);
    setDone(false);
    try {
      await invoke("download_novel_chapters", {
        url: novelUrl,
        workingFolder,
        chapterIds: Array.from(selectedIds),
        templateName: "newtoki",
        force: forceOverwrite,
        chapterDelayMs,
      });
    } catch (e) {
      const msg = String(e);
      if (!msg.includes("使用者已停止下載")) {
        alert(`下載失敗: ${e}`);
      }
      setIsDownloading(false);
    }
  }

  async function stopDownload() {
    try {
      await invoke("cancel_download");
    } catch (e) {
      alert(`停止失敗: ${e}`);
    }
  }

  return (
    <div className="app-shell">
      <NavBar current={page} onChange={setPage} />
      <main className="app-content">
        {page === "home" && (
          <HomePage
            novelUrl={novelUrl}
            workingFolder={workingFolder}
            isDownloading={isDownloading}
            isListing={isListing}
            fetcherOpen={fetcherOpen}
            selectedCount={selectedIds.size}
            chapterList={chapterList}
            novelTitle={novelTitle}
            selectedIds={selectedIds}
            progress={progress}
            done={done}
            onNovelUrlChange={handleNovelUrlChange}
            onSelectFolder={selectFolder}
            onOpenFetcher={openFetcher}
            onListChapters={listChapters}
            onDownloadSelected={downloadSelected}
            onStopDownload={stopDownload}
            onToggleChapter={toggleChapter}
            onSelectAll={selectAllChapters}
            onDeselectAll={deselectAllChapters}
          />
        )}
        {page === "settings" && (
          <SettingsPage
            workingFolder={workingFolder}
            forceOverwrite={forceOverwrite}
            chapterDelayMs={chapterDelayMs}
            isDownloading={isDownloading}
            isListing={isListing}
            onSelectFolder={selectFolder}
            onForceOverwriteChange={setForceOverwrite}
            onChapterDelayChange={setChapterDelayMs}
          />
        )}
        {page === "debug" && (
          <DebugPage
            fetcherOpen={fetcherOpen}
            isDownloading={isDownloading}
            isListing={isListing}
            progress={progress}
            done={done}
            onProbeCurrentPage={probeCurrentPage}
            onStopDownload={stopDownload}
          />
        )}
        {page === "help" && <HelpPage />}
      </main>
    </div>
  );
}

export default App;
