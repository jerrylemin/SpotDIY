import { createContext, useContext } from "react";

import type { LayoutProfile, SettingsSnapshot, Theme } from "../../types/domain";
import { parseThemeDefinition, type SpotThemeDefinition } from "./theme-schema";

export const SETTINGS_QUERY_KEY = ["settings"] as const;

export type ResolvedTheme = "dark" | "light";

export interface ThemeContextValue {
  settings: SettingsSnapshot | undefined;
  isLoading: boolean;
  theme: Theme;
  resolvedTheme: ResolvedTheme;
  resolvedSystemTheme: ResolvedTheme;
  error: string | null;
  setTheme: (theme: Theme) => Promise<SettingsSnapshot>;
  setLayoutProfile: (profile: LayoutProfile) => Promise<SettingsSnapshot>;
  importCustomTheme: (theme: unknown) => Promise<SettingsSnapshot>;
  exportCustomTheme: () => string | null;
  resetCustomTheme: () => Promise<SettingsSnapshot>;
  previewTheme: (theme: SpotThemeDefinition | null) => void;
  stopThemePreview: () => void;
  dynamicAccentEnabled: boolean;
  setDynamicAccent: (enabled: boolean) => void;
}

export const ThemeContext = createContext<ThemeContextValue | null>(null);

export function resolveTheme(theme: Theme, system: ResolvedTheme, customTheme?: SpotThemeDefinition | null): ResolvedTheme {
  if (theme === "system") return system;
  if (theme === "custom") {
    try {
      return customTheme ? parseThemeDefinition(customTheme).baseMode : "dark";
    } catch {
      return "dark";
    }
  }
  return theme;
}

export function useTheme(): ThemeContextValue {
  const context = useContext(ThemeContext);
  if (!context) throw new Error("useTheme must be used within ThemeController");
  return context;
}
