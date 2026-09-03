import type { SpotThemeDefinition } from "./theme-schema";

export const DARK_THEME: SpotThemeDefinition = {
  schemaVersion: 1,
  name: "SpotDIY Dark",
  baseMode: "dark",
  tokens: {
    background: "#101113",
    surface: "#17181D",
    surfaceRaised: "#1D1E24",
    surfaceSoft: "#22232A",
    text: "#F3F1EC",
    textMuted: "#A8A7AE",
    textSubtle: "#85848C",
    border: "#2E2F36",
    borderStrong: "#4B4C55",
    accent: "#D7FF60",
    accentContrast: "#151617",
    success: "#81E2D0",
    warning: "#FFB570",
    danger: "#FF806F",
    info: "#8E7BFF",
  },
};

export const LIGHT_THEME: SpotThemeDefinition = {
  schemaVersion: 1,
  name: "SpotDIY Light",
  baseMode: "light",
  tokens: {
    background: "#F6F7F2",
    surface: "#FFFFFF",
    surfaceRaised: "#FFFFFF",
    surfaceSoft: "#EDF0E8",
    text: "#161719",
    textMuted: "#53565B",
    textSubtle: "#6D7176",
    border: "#D6D9D1",
    borderStrong: "#A7ADA4",
    accent: "#567800",
    accentContrast: "#FFFFFF",
    success: "#167C62",
    warning: "#A45700",
    danger: "#BD3027",
    info: "#5C4DE0",
  },
};

export const BUILT_IN_THEMES = {
  dark: DARK_THEME,
  light: LIGHT_THEME,
} as const;
