import { LogPanel } from "../components/LogPanel/LogPanel";
import "../components/ControlPanel/ControlPanel.css";

interface DebugPageProps {
  fetcherOpen: boolean;
  isDownloading: boolean;
  isListing: boolean;
  progress: string[];
  done: boolean;
  onProbeCurrentPage: () => void;
  onStopDownload: () => void;
}

export function DebugPage({
  fetcherOpen,
  isDownloading,
  isListing,
  progress,
  done,
  onProbeCurrentPage,
  onStopDownload,
}: DebugPageProps) {
  return (
    <div className="page-layout page-layout--debug">
      <section className="control-panel page-section page-section--compact">
        <h2 className="page-section__title">Debug</h2>
        <p className="hint">
          當站點封鎖 F12 或需要分析抓取視窗中的頁面結構時，可使用探測功能。請先在主頁開啟抓取視窗並導航至目標頁面。
        </p>
        <div className="row actions-row">
          <button
            type="button"
            onClick={onProbeCurrentPage}
            disabled={isDownloading || isListing || !fetcherOpen}
          >
            探測目前抓取頁（免 F12）
          </button>
        </div>
        {!fetcherOpen && (
          <p className="hint">目前尚未開啟抓取視窗。</p>
        )}
      </section>

      <div className="debug-log">
        <LogPanel
          lines={progress}
          done={done}
          isDownloading={isDownloading}
          onStopDownload={onStopDownload}
        />
      </div>
    </div>
  );
}
