import type { ChapterListItem } from "../../types/novel";
import { Panel } from "../Panel/Panel";
import "./ChapterPanel.css";

interface ChapterPanelProps {
  chapters: ChapterListItem[];
  novelTitle: string;
  selectedIds: Set<string>;
  onToggleChapter: (id: string) => void;
  onSelectAll: () => void;
  onDeselectAll: () => void;
}

export function ChapterPanel({
  chapters,
  novelTitle,
  selectedIds,
  onToggleChapter,
  onSelectAll,
  onDeselectAll,
}: ChapterPanelProps) {
  const meta =
    chapters.length > 0 ? (
      <span className="panel-meta">
        {novelTitle} · {chapters.length} 章 · 已選 {selectedIds.size}
      </span>
    ) : undefined;

  return (
    <Panel title="章節列表" meta={meta} fill>
      {chapters.length === 0 ? (
        <p className="empty-state">按「② 列出章節」後，章節會顯示在此</p>
      ) : (
        <>
          <div className="chapter-toolbar">
            <button type="button" className="chapter-toolbar__btn" onClick={onSelectAll}>
              全選
            </button>
            <span className="chapter-toolbar__sep">·</span>
            <button type="button" className="chapter-toolbar__btn" onClick={onDeselectAll}>
              取消全選
            </button>
          </div>
          {chapters.map((ch) => {
            const locked = ch.requiresLogin;
            const suffix = locked ? " 🔒" : ch.localStatus === "exists" ? " ✓" : "";
            return (
              <label
                key={ch.id}
                className={`chapter-row${locked ? " chapter-row--locked" : ""}`}
              >
                <input
                  type="checkbox"
                  checked={selectedIds.has(ch.id)}
                  disabled={locked}
                  onChange={() => onToggleChapter(ch.id)}
                />
                <span>
                  {String(ch.index).padStart(4, "0")} {ch.title}
                  {suffix}
                </span>
              </label>
            );
          })}
        </>
      )}
    </Panel>
  );
}
