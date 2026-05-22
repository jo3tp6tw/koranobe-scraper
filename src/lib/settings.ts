export interface AppSettings {
  novelUrl: string;
  workingFolder: string;
  forceOverwrite: boolean;
  chapterDelayMs: number;
}

const STORAGE_KEY = "newtoki-scraper-settings";

export function loadSettings(): Partial<AppSettings> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as Partial<AppSettings>;
    return {
      novelUrl: typeof parsed.novelUrl === "string" ? parsed.novelUrl : undefined,
      workingFolder:
        typeof parsed.workingFolder === "string" ? parsed.workingFolder : undefined,
      forceOverwrite:
        typeof parsed.forceOverwrite === "boolean" ? parsed.forceOverwrite : undefined,
      chapterDelayMs:
        typeof parsed.chapterDelayMs === "number" ? parsed.chapterDelayMs : undefined,
    };
  } catch {
    return {};
  }
}

export function saveSettings(settings: AppSettings): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
}
