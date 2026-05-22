import { Panel } from "../Panel/Panel";
import { LogDownloadControl } from "./LogDownloadControl";
import "./LogPanel.css";

interface LogPanelProps {
  lines: string[];
  done: boolean;
  isDownloading?: boolean;
  onStopDownload?: () => void;
}

export function LogPanel({
  lines,
  done,
  isDownloading = false,
  onStopDownload,
}: LogPanelProps) {
  const headerMeta =
    isDownloading && onStopDownload ? (
      <div className="log-panel-header-meta">
        <LogDownloadControl onStop={onStopDownload} />
      </div>
    ) : done ? (
      <span className="done-badge">下載完成</span>
    ) : undefined;

  return (
    <Panel
      title="Log"
      meta={headerMeta}
      fill
      panelClassName="panel--terminal"
      bodyClassName="panel-body--mono"
    >
      {lines.length === 0 ? (
        <p className="empty-state">操作與下載進度會顯示在此</p>
      ) : (
        lines.map((msg, i) => <div key={i}>{msg}</div>)
      )}
    </Panel>
  );
}
