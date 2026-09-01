import type { SVGProps } from "react";

export type SpotIconName =
  | "home"
  | "search"
  | "library"
  | "lyrics"
  | "playlist"
  | "download"
  | "settings"
  | "play"
  | "pause"
  | "previous"
  | "next"
  | "queue"
  | "command"
  | "folder"
  | "spark"
  | "chevron"
  | "close"
  | "arrow"
  | "refresh"
  | "trash"
  | "file"
  | "alert"
  | "more"
  | "check"
  | "info"
  | "pin"
  | "bookmark"
  | "edit"
  | "theme"
  | "layout"
  | "sun"
  | "moon"
  | "system"
  | "expand"
  | "collapse"
  | "volume"
  | "mute"
  | "shuffle"
  | "repeat"
  | "device";

interface SpotIconProps extends SVGProps<SVGSVGElement> {
  name: SpotIconName;
  size?: number;
}

const paths: Record<SpotIconName, string> = {
  home: "M4 10.5 12 4l8 6.5v8a1.5 1.5 0 0 1-1.5 1.5h-4v-5h-5v5h-4A1.5 1.5 0 0 1 4 18.5z",
  search: "m19 19-4.4-4.4M10.8 17a6.2 6.2 0 1 1 0-12.4 6.2 6.2 0 0 1 0 12.4Z",
  library: "M4 5.5A1.5 1.5 0 0 1 5.5 4H18a2 2 0 0 1 2 2v12.5A1.5 1.5 0 0 1 18.5 20H5.5A1.5 1.5 0 0 1 4 18.5zM8 4v16M12 4v16",
  lyrics: "M7 4.5h10A1.5 1.5 0 0 1 18.5 6v12A1.5 1.5 0 0 1 17 19.5H7A1.5 1.5 0 0 1 5.5 18V6A1.5 1.5 0 0 1 7 4.5ZM8.5 8h7M8.5 12h7M8.5 16h4",
  playlist: "M5 6h9M5 11h9M5 16h5m8-8v8m0 0-3-3m3 3 3-3",
  download: "M12 4v10m0 0 3.5-3.5M12 14 8.5 10.5M5 18.5h14",
  settings: "M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7Zm0-4v2m0 11v2M5.6 6.6 7 8m10-1.4-1.4 1.4M5.6 18.4 7 17m10 1.4-1.4-1.4M4 12h2m12 0h2",
  play: "m9 6 9 6-9 6z",
  pause: "M8 6v12M16 6v12",
  previous: "m17 6-7 6 7 6M8 6v12",
  next: "m7 6 7 6-7 6m9-12v12",
  queue: "M4 7h12M4 12h8M4 17h6m12-8-3 3m0 0-3-3m3 3V5",
  command: "M7.5 4A3.5 3.5 0 1 0 11 7.5V18a3.5 3.5 0 1 0 3.5-3.5H4A3.5 3.5 0 1 1 7.5 11H18a3.5 3.5 0 1 1-3.5 3.5",
  folder: "M3.5 7.5A1.5 1.5 0 0 1 5 6h4l1.5 2H19a1.5 1.5 0 0 1 1.5 1.5v8A1.5 1.5 0 0 1 19 19H5a1.5 1.5 0 0 1-1.5-1.5z",
  spark: "m12 3 1.4 5.6L19 10l-5.6 1.4L12 17l-1.4-5.6L5 10l5.6-1.4z",
  chevron: "m7 9 5 5 5-5",
  close: "m6 6 12 12M18 6 6 18",
  arrow: "M5 12h13m-5-5 5 5-5 5",
  refresh: "M20 11a8 8 0 0 0-14.9-3M4 5v4h4m-4 3a8 8 0 0 0 14.9 3M20 19v-4h-4",
  trash: "M5 7h14m-9 4v5m4-5v5M9 7V5h6v2m-9 0 1 13h10l1-13",
  file: "M6 3.5h8l4 4V20a.5.5 0 0 1-.5.5h-11A.5.5 0 0 1 6 20zM14 3.5V8h4",
  alert: "M12 4 21 20H3zM12 9v5m0 3h.01",
  more: "M5 12h.01M12 12h.01M19 12h.01",
  check: "m5 12 4.5 4.5L19 7",
  info: "M12 10v6m0-9h.01M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Z",
  pin: "M8 4h8m-7 0v5l-3 4h5v7l2-3 2 3v-7h5l-3-4V4",
  bookmark: "M6 4.5A1.5 1.5 0 0 1 7.5 3h9A1.5 1.5 0 0 1 18 4.5V21l-6-3-6 3z",
  edit: "m5 16-1 4 4-1L19 8a2.8 2.8 0 0 0-4-4zM13 7l4 4",
  theme: "M12 3a9 9 0 1 0 9 9c-5 1-9-3-9-9Zm6.5 1.5.01 0M20 8h.01M17 18h.01",
  layout: "M4 5.5A1.5 1.5 0 0 1 5.5 4h13A1.5 1.5 0 0 1 20 5.5v13a1.5 1.5 0 0 1-1.5 1.5h-13A1.5 1.5 0 0 1 4 18.5zM4 9h16M10 9v11",
  sun: "M12 5V3m0 18v-2M5 12H3m18 0h-2M5.6 5.6 4.2 4.2m15.6 15.6-1.4-1.4M18.4 5.6l1.4-1.4M4.2 19.8l1.4-1.4M16.5 12a4.5 4.5 0 1 1-9 0 4.5 4.5 0 0 1 9 0Z",
  moon: "M20 15.4A8.5 8.5 0 0 1 8.6 4a8.5 8.5 0 1 0 11.4 11.4Z",
  system: "M5 5.5A1.5 1.5 0 0 1 6.5 4h11A1.5 1.5 0 0 1 19 5.5v8A1.5 1.5 0 0 1 17.5 15h-11A1.5 1.5 0 0 1 5 13.5zM9 20h6m-3-5v5",
  expand: "M8 3H3v5m13-5h5v5M8 21H3v-5m18 0v5h-5",
  collapse: "M9 3v6H3m12-6v6h6M9 21v-6H3m12 6v-6h6",
  volume: "M5 10v4h3l4 4V6L8 10zm11.5 2a4.5 4.5 0 0 0-2.5-4m0 8a4.5 4.5 0 0 0 2.5-4",
  mute: "M5 10v4h3l4 4V6L8 10zm9-2 5 8m0-8-5 8",
  shuffle: "M16 4h4v4m0 12h-4v-4M4 7h3c2.5 0 4.1.7 5.4 2.7L17 16c1.1 1.5 2 2 3 2m0-12c-1 0-1.9.5-3 2l-1.6 2.2M4 17h3c2.5 0 4.1-.7 5.4-2.7L14 12",
  repeat: "M17 17H7a3 3 0 0 1-3-3V9m0 0 3 3M4 9l-3 3M7 7h10a3 3 0 0 1 3 3v5m0 0-3-3m3 3 3-3",
  device: "M5 6.5A1.5 1.5 0 0 1 6.5 5h11A1.5 1.5 0 0 1 19 6.5v7A1.5 1.5 0 0 1 17.5 15H13l-2 4-2-4H6.5A1.5 1.5 0 0 1 5 13.5z",
};

export const spotIconNames = Object.keys(paths) as SpotIconName[];

export function SpotIcon({ name, size = 20, strokeWidth = 1.8, ...props }: SpotIconProps) {
  return (
    <svg
      aria-hidden="true"
      fill="none"
      height={size}
      viewBox="0 0 24 24"
      width={size}
      {...props}
    >
      <path d={paths[name]} stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" strokeWidth={strokeWidth} />
    </svg>
  );
}

export function SpotLogo({ size = 32 }: { size?: number }) {
  return (
    <svg aria-hidden="true" height={size} viewBox="0 0 256 256" width={size}>
      <rect fill="currentColor" height="256" rx="64" width="256" />
      <path d="M58 75c33-25 72-25 105 0M52 111c36-23 72-23 110 0" fill="none" stroke="var(--color-accent-contrast)" strokeLinecap="round" strokeWidth="18" />
      <path d="M50 148c30-19 60-19 91-1l-23 12 57 32-5-65-21 12c-37-24-70-24-99-5" fill="none" stroke="var(--color-accent-contrast)" strokeLinecap="round" strokeLinejoin="round" strokeWidth="18" />
    </svg>
  );
}
