import type { SVGProps } from "react";

export type SpotIconName =
  | "home"
  | "search"
  | "library"
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
  | "arrow";

interface SpotIconProps extends SVGProps<SVGSVGElement> {
  name: SpotIconName;
  size?: number;
}

const paths: Record<SpotIconName, string> = {
  home: "M4 10.5 12 4l8 6.5v8a1.5 1.5 0 0 1-1.5 1.5h-4v-5h-5v5h-4A1.5 1.5 0 0 1 4 18.5z",
  search: "m19 19-4.4-4.4M10.8 17a6.2 6.2 0 1 1 0-12.4 6.2 6.2 0 0 1 0 12.4Z",
  library: "M4 5.5A1.5 1.5 0 0 1 5.5 4H18a2 2 0 0 1 2 2v12.5A1.5 1.5 0 0 1 18.5 20H5.5A1.5 1.5 0 0 1 4 18.5zM8 4v16M12 4v16",
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
};

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
      <path d="M58 75c33-25 72-25 105 0M52 111c36-23 72-23 110 0" fill="none" stroke="#17181d" strokeLinecap="round" strokeWidth="18" />
      <path d="M50 148c30-19 60-19 91-1l-23 12 57 32-5-65-21 12c-37-24-70-24-99-5" fill="none" stroke="#17181d" strokeLinecap="round" strokeLinejoin="round" strokeWidth="18" />
    </svg>
  );
}
