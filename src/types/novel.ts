export interface ChapterListItem {
  index: number;
  id: string;
  title: string;
  url: string;
  requiresLogin: boolean;
  localStatus: "none" | "exists";
}

export interface ListNovelChaptersResponse {
  title: string;
  author: string;
  introduction: string;
  tags: string[];
  chapters: ChapterListItem[];
  defaultChapterDelayMs: number;
}

export interface ProgressEvent {
  stage: string;
  url?: string;
  info?: { title?: string; chapters?: unknown[] };
  chapter?: {
    index: number;
    total: number;
    title: string;
    id: string;
    charCount: number;
    status: string;
  };
  items?: { index: number; title: string; id: string; reason: string }[];
  folder?: string;
  message?: string;
}
