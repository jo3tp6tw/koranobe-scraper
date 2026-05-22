import { ChapterPanel } from "../components/ChapterPanel/ChapterPanel";
import { ControlPanel } from "../components/ControlPanel/ControlPanel";
import { LogPanel } from "../components/LogPanel/LogPanel";
import type { ChapterListItem } from "../types/novel";

interface HomePageProps {
  novelUrl: string;
  workingFolder: string;
  isDownloading: boolean;
  isListing: boolean;
  fetcherOpen: boolean;
  selectedCount: number;
  chapterList: ChapterListItem[];
  novelTitle: string;
  selectedIds: Set<string>;
  progress: string[];
  done: boolean;
  onNovelUrlChange: (url: string) => void;
  onSelectFolder: () => void;
  onOpenFetcher: () => void;
  onListChapters: () => void;
  onDownloadSelected: () => void;
  onStopDownload: () => void;
  onToggleChapter: (id: string) => void;
  onSelectAll: () => void;
  onDeselectAll: () => void;
}

export function HomePage({
  novelUrl,
  workingFolder,
  isDownloading,
  isListing,
  fetcherOpen,
  selectedCount,
  chapterList,
  novelTitle,
  selectedIds,
  progress,
  done,
  onNovelUrlChange,
  onSelectFolder,
  onOpenFetcher,
  onListChapters,
  onDownloadSelected,
  onStopDownload,
  onToggleChapter,
  onSelectAll,
  onDeselectAll,
}: HomePageProps) {
  return (
    <div className="page-layout page-layout--split">
      <div className="app-column app-column--left">
        <ControlPanel
          novelUrl={novelUrl}
          workingFolder={workingFolder}
          isDownloading={isDownloading}
          isListing={isListing}
          fetcherOpen={fetcherOpen}
          selectedCount={selectedCount}
          onNovelUrlChange={onNovelUrlChange}
          onSelectFolder={onSelectFolder}
          onOpenFetcher={onOpenFetcher}
          onListChapters={onListChapters}
          onDownloadSelected={onDownloadSelected}
          onStopDownload={onStopDownload}
        />
        <ChapterPanel
          chapters={chapterList}
          novelTitle={novelTitle}
          selectedIds={selectedIds}
          onToggleChapter={onToggleChapter}
          onSelectAll={onSelectAll}
          onDeselectAll={onDeselectAll}
        />
      </div>

      <div className="app-column app-column--right">
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
