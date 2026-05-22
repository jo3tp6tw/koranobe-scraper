import { FolderField } from "../FolderField/FolderField";
import "./ControlPanel.css";

interface ControlPanelProps {
  novelUrl: string;
  workingFolder: string;
  isDownloading: boolean;
  isListing: boolean;
  fetcherOpen: boolean;
  selectedCount: number;
  onNovelUrlChange: (url: string) => void;
  onSelectFolder: () => void;
  onOpenFetcher: () => void;
  onListChapters: () => void;
  onDownloadSelected: () => void;
  onStopDownload: () => void;
}

export function ControlPanel({
  novelUrl,
  workingFolder,
  isDownloading,
  isListing,
  fetcherOpen,
  selectedCount,
  onNovelUrlChange,
  onSelectFolder,
  onOpenFetcher,
  onListChapters,
  onDownloadSelected,
  onStopDownload,
}: ControlPanelProps) {
  const missingFolder = !workingFolder;

  return (
    <section className="control-panel">
      <div className="field">
        <label>小說 URL</label>
        <input
          type="text"
          value={novelUrl}
          onChange={(e) => onNovelUrlChange(e.currentTarget.value)}
          placeholder="https://sbxh1.com/novel/..."
        />
      </div>

      <div className="field">
        <label>存放目錄</label>
        <FolderField value={workingFolder} onSelect={onSelectFolder} />
      </div>

      <div className="row actions-row">
        <button
          type="button"
          onClick={onOpenFetcher}
          disabled={isDownloading || isListing || !novelUrl}
        >
          ① 開啟抓取視窗
        </button>
        <button
          type="button"
          onClick={onListChapters}
          disabled={isDownloading || isListing || !novelUrl || !fetcherOpen}
        >
          {isListing ? "列出中..." : "② 列出章節"}
        </button>
      </div>

      <div className="row actions-row">
        <button
          type="button"
          className="primary"
          onClick={onDownloadSelected}
          disabled={
            isDownloading ||
            isListing ||
            !novelUrl ||
            missingFolder ||
            !fetcherOpen ||
            selectedCount === 0
          }
        >
          {isDownloading ? "下載中..." : `③ 下載已選 (${selectedCount})`}
        </button>
      </div>

      {isDownloading && (
        <div className="row actions-row">
          <button type="button" className="danger" onClick={onStopDownload}>
            強制停止
          </button>
        </div>
      )}
    </section>
  );
}
