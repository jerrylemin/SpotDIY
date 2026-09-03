import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";

import {
  getSettingsSnapshot,
  IpcError,
  isTauriRuntime,
  setSetting,
} from "../../services/ipc";
import { usePlayback } from "../../hooks/usePlayback";
import type { LayoutProfile, SettingValue, SettingsSnapshot, Theme } from "../../types/domain";
import { DARK_THEME } from "./theme-presets";
import {
  parseThemeDefinition,
  serializeThemeDefinition,
  themeCssVariables,
  type SpotThemeDefinition,
} from "./theme-schema";
import { sampleAccentFromPixels } from "./theme-studio/dynamic-accent";

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
  previewTheme: (theme: SpotThemeDefinition | null) => void;
  stopThemePreview: () => void;
  dynamicAccentEnabled: boolean;
  setDynamicAccent: (enabled: boolean) => void;
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

function applyRootAppearance(
  settings: SettingsSnapshot | undefined,
  resolvedSystemTheme: ResolvedTheme,
  previewTheme: SpotThemeDefinition | null,
  dynamicAccent: { accent: string; accentContrast: string } | null,
) {
  if (typeof document === "undefined") {
    return;
  }

  const root = document.documentElement;
  const selectedTheme = previewTheme ? "custom" : settings?.theme ?? "dark";
  const layoutProfile = settings?.layoutProfile ?? "comfortable";
  const customTheme = previewTheme ?? (selectedTheme === "custom" && settings?.customTheme
    ? parseThemeDefinition(settings.customTheme)
    : null);
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
  if (dynamicAccent) {
    root.style.setProperty("--color-accent", dynamicAccent.accent);
    root.style.setProperty("--color-accent-contrast", dynamicAccent.accentContrast);
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
  const [previewTheme, setPreviewTheme] = useState<SpotThemeDefinition | null>(null);
  const [dynamicAccentEnabled, setDynamicAccentEnabled] = useState(false);
  const [dynamicAccent, setDynamicAccent] = useState<{ accent: string; accentContrast: string } | null>(null);
  const playback = usePlayback();

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
    let active = true;
    const artworkPath = playback.snapshot.artworkPath;
    if (!dynamicAccentEnabled || !artworkPath || !isTauriRuntime() || typeof Image === "undefined") {
      setDynamicAccent(null);
      return undefined;
    }
    const image = new Image();
    image.onload = () => {
      if (!active) return;
      const canvas = document.createElement("canvas");
      canvas.width = 32;
      canvas.height = 32;
      const context = canvas.getContext("2d", { willReadFrequently: true });
      if (!context) return;
      try {
        context.drawImage(image, 0, 0, 32, 32);
        const root = document.documentElement;
        const styles = getComputedStyle(root);
        setDynamicAccent(sampleAccentFromPixels(
          context.getImageData(0, 0, 32, 32).data,
          styles.getPropertyValue("--color-bg").trim(),
          styles.getPropertyValue("--color-surface").trim(),
        ));
      } catch {
        setDynamicAccent(null);
      }
    };
    image.onerror = () => { if (active) setDynamicAccent(null); };
    image.src = convertFileSrc(artworkPath);
    return () => {
      active = false;
      image.onload = null;
      image.onerror = null;
    };
  }, [dynamicAccentEnabled, playback.snapshot.artworkPath, previewTheme, resolvedSystemTheme, settingsQuery.data]);

  useEffect(() => {
    try {
      applyRootAppearance(settingsQuery.data, resolvedSystemTheme, previewTheme, dynamicAccent);
    } catch (error) {
      const root = document.documentElement;
      root.dataset.theme = "dark";
      root.style.colorScheme = "dark";
      for (const variableName of customVariableNames) {
        root.style.removeProperty(variableName);
      }
      setActionError(errorMessage(error));
    }
  }, [dynamicAccent, previewTheme, resolvedSystemTheme, settingsQuery.data]);

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
  const previewThemeForSession = useCallback((theme: SpotThemeDefinition | null) => setPreviewTheme(theme), []);
  const stopThemePreview = useCallback(() => setPreviewTheme(null), []);
  const setDynamicAccentForSession = useCallback((enabled: boolean) => {
    setDynamicAccentEnabled(enabled);
    if (!enabled) setDynamicAccent(null);
  }, []);

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
    previewTheme: previewThemeForSession,
    stopThemePreview,
    dynamicAccentEnabled,
    setDynamicAccent: setDynamicAccentForSession,
  }), [actionError, dynamicAccentEnabled, exportCustomTheme, importCustomTheme, previewThemeForSession, resolvedSystemTheme, resolvedTheme, resetCustomTheme, setDynamicAccentForSession, setLayoutProfile, setTheme, settingsQuery.data, settingsQuery.error, settingsQuery.isLoading, stopThemePreview, theme, validationError]);

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

export function useTheme() {
  const context = useContext(ThemeContext);
  if (!context) {
    throw new Error("useTheme must be used within ThemeController");
  }
  return context;
}
