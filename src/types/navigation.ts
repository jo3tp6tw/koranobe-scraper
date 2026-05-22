export type AppPage = "home" | "settings" | "debug" | "help";

export const APP_PAGES: {
  id: AppPage;
  label: string;
  iconSrc: string;
  iconState: string;
}[] = [
  {
    id: "home",
    label: "主頁",
    iconSrc: "/lordicon/system-solid-41-home-hover-pinch.json",
    iconState: "hover-pinch",
  },
  {
    id: "settings",
    label: "設定",
    iconSrc: "/lordicon/system-solid-63-settings-cog-hover-cog-4.json",
    iconState: "hover-cog-4",
  },
  {
    id: "debug",
    label: "Debug",
    iconSrc: "/lordicon/system-solid-34-code-hover-code.json",
    iconState: "hover-code",
  },
  {
    id: "help",
    label: "說明",
    iconSrc: "/lordicon/system-solid-28-info-hover-info.json",
    iconState: "hover-info",
  },
];
