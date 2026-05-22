import { FolderField } from "../components/FolderField/FolderField";
import "../components/ControlPanel/ControlPanel.css";

interface SettingsPageProps {
  workingFolder: string;
  forceOverwrite: boolean;
  chapterDelayMs: number;
  isDownloading: boolean;
  isListing: boolean;
  onSelectFolder: () => void;
  onForceOverwriteChange: (value: boolean) => void;
  onChapterDelayChange: (value: number) => void;
}

export function SettingsPage({
  workingFolder,
  forceOverwrite,
  chapterDelayMs,
  isDownloading,
  isListing,
  onSelectFolder,
  onForceOverwriteChange,
  onChapterDelayChange,
}: SettingsPageProps) {
  const busy = isDownloading || isListing;

  return (
    <div className="page-layout page-layout--single">
      <section className="control-panel page-section">
        <h2 className="page-section__title">設定</h2>

        <div className="field">
          <label>存放目錄</label>
          <FolderField value={workingFolder} onSelect={onSelectFolder} />
          <p className="hint">下載的章節檔案會儲存到此目錄。</p>
        </div>

        <div className="field">
          <label className="checkbox-row" htmlFor="force-overwrite">
            <input
              id="force-overwrite"
              type="checkbox"
              checked={forceOverwrite}
              onChange={(e) => onForceOverwriteChange(e.currentTarget.checked)}
              disabled={busy}
            />
            <span>強制覆蓋已存在章節</span>
          </label>
          <p className="hint">啟用後，即使本地已有章節檔案也會重新下載。</p>
        </div>

        <div className="field">
          <label htmlFor="chapter-delay">章節間隔（毫秒）</label>
          <input
            id="chapter-delay"
            type="number"
            min={0}
            step={100}
            value={chapterDelayMs}
            onChange={(e) =>
              onChapterDelayChange(Math.max(0, Number(e.currentTarget.value) || 0))
            }
            disabled={busy}
          />
          <p className="hint">每章下載之間的等待時間，避免請求過於頻繁。</p>
        </div>
      </section>
    </div>
  );
}
