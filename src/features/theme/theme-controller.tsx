import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import {
  getSettingsSnapshot,
  IpcError,
  setSetting,
} from "../../services/ipc";
import type { LayoutProfile, SettingValue, SettingsSnapshot, Theme } from "../../types/domain";
import { DARK_THEME } from "./theme-presets";
import {
  parseThemeDefinition,
  serializeThemeDefinition,
  themeCssVariables,
  type SpotThemeDefinition,
} from "./theme-schema";

export const SETTINGS_QUERY_KEY = ["settings"] as const;

export type ResolvedTheme = "dark" | "light";

interface ThemeContextValue {
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
}

const ThemeContext = createContext<ThemeContextValue | null>(null);
const customVariableNames = Object.keys(themeCssVariables(DARK_THEME));

function systemTheme(): ResolvedTheme {
  return typeof window !== "undefined" && typeof window.matchMedia === "function" && window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

function errorMessage(error: unknown): string {
  if (error instanceof IpcError && error.cause instanceof Error && error.cause.message) {
    return `${error.message} ${error.cause.message}`;
  }
  if (error instanceof Error && error.message) {
    return error.message;
  }
  return "SpotDIY could not update appearance settings.";
}

function applyRootAppearance(settings: SettingsSnapshot | undefined, resolvedSystemTheme: ResolvedTheme) {
  if (typeof document === "undefined") {
    return;
  }

  const root = document.documentElement;
  const selectedTheme = settings?.theme ?? "dark";
  const layoutProfile = settings?.layoutProfile ?? "comfortable";
  const customTheme = selectedTheme === "custom" && settings?.customTheme
    ? parseThemeDefinition(settings.customTheme)
    : null;
  const resolvedTheme = customTheme?.baseMode ?? (selectedTheme === "system" ? resolvedSystemTheme : selectedTheme === "light" ? "light" : "dark");

  root.dataset.layout = layoutProfile;
  root.style.colorScheme = resolvedTheme;
  for (const variableName of customVariableNames) {
    root.style.removeProperty(variableName);
  }

  if (customTheme) {
    root.dataset.theme = "custom";
    for (const [variableName, value] of Object.entries(themeCssVariables(customTheme))) {
      root.style.setProperty(variableName, value);
    }
  } else {
    root.dataset.theme = resolvedTheme;
  }
}

export function resolveTheme(theme: Theme, system: ResolvedTheme, customTheme?: SpotThemeDefinition | null): ResolvedTheme {
  if (theme === "system") {
    return system;
  }
  if (theme === "custom") {
    try {
      return customTheme ? parseThemeDefinition(customTheme).baseMode : "dark";
    } catch {
      return "dark";
    }
  }
  return theme;
}

export function ThemeController({ children }: { children: ReactNode }) {
  const queryClient = useQueryClient();
  const settingsQuery = useQuery({
    queryKey: SETTINGS_QUERY_KEY,
    queryFn: getSettingsSnapshot,
    staleTime: Number.POSITIVE_INFINITY,
    retry: 1,
  });
  const [resolvedSystemTheme, setResolvedSystemTheme] = useState<ResolvedTheme>(systemTheme);
  const [actionError, setActionError] = useState<string | null>(null);

  useEffect(() => {
    if (typeof window === "undefined") {
      return undefined;
    }
    if (typeof window.matchMedia !== "function") {
      return undefined;
    }
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (event: MediaQueryListEvent) => setResolvedSystemTheme(event.matches ? "dark" : "light");
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, []);

  useEffect(() => {
    try {
      applyRootAppearance(settingsQuery.data, resolvedSystemTheme);
    } catch (error) {
      const root = document.documentElement;
      root.dataset.theme = "dark";
      root.style.colorScheme = "dark";
      for (const variableName of customVariableNames) {
        root.style.removeProperty(variableName);
      }
      setActionError(errorMessage(error));
    }
  }, [resolvedSystemTheme, settingsQuery.data]);

  const updateSetting = useCallback(async (setting: SettingValue) => {
    setActionError(null);
    try {
      const next = await setSetting(setting);
      queryClient.setQueryData(SETTINGS_QUERY_KEY, next);
      return next;
    } catch (error) {
      setActionError(errorMessage(error));
      throw error;
    }
  }, [queryClient]);

  const setTheme = useCallback((theme: Theme) => updateSetting({ key: "theme", value: theme }), [updateSetting]);
  const setLayoutProfile = useCallback((layoutProfile: LayoutProfile) => updateSetting({ key: "layoutProfile", value: layoutProfile }), [updateSetting]);

  const importCustomTheme = useCallback(async (theme: unknown) => {
    const validated = parseThemeDefinition(theme);
    await updateSetting({ key: "customTheme", value: validated });
    return updateSetting({ key: "theme", value: "custom" });
  }, [updateSetting]);

  const exportCustomTheme = useCallback(() => {
    const customTheme = settingsQuery.data?.customTheme;
    return customTheme ? serializeThemeDefinition(customTheme) : null;
  }, [settingsQuery.data?.customTheme]);

  const resetCustomTheme = useCallback(async () => {
    if (settingsQuery.data?.theme === "custom") {
      await updateSetting({ key: "theme", value: "dark" });
    }
    return updateSetting({ key: "customTheme", value: null });
  }, [settingsQuery.data?.theme, updateSetting]);

  const theme = settingsQuery.data?.theme ?? "dark";
  const customTheme = settingsQuery.data?.customTheme;
  const resolvedTheme = resolveTheme(theme, resolvedSystemTheme, customTheme);
  const validationError = theme === "custom" && !customTheme
    ? "Custom theme is selected but no valid definition is available. Dark theme is active until one is imported."
    : null;
  const value = useMemo<ThemeContextValue>(() => ({
    settings: settingsQuery.data,
    isLoading: settingsQuery.isLoading,
    theme,
    resolvedTheme,
    resolvedSystemTheme,
    error: validationError ?? actionError ?? (settingsQuery.error ? errorMessage(settingsQuery.error) : null),
    setTheme,
    setLayoutProfile,
    importCustomTheme,
    exportCustomTheme,
    resetCustomTheme,
  }), [actionError, exportCustomTheme, importCustomTheme, resolvedSystemTheme, resolvedTheme, resetCustomTheme, setLayoutProfile, setTheme, settingsQuery.data, settingsQuery.error, settingsQuery.isLoading, theme, validationError]);

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

export function useTheme() {
  const context = useContext(ThemeContext);
  if (!context) {
    throw new Error("useTheme must be used within ThemeController");
  }
  return context;
}
