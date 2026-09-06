import { useEffect, useMemo, useState } from "react";

import { LayoutWorkspace } from "../layout/LayoutWorkspace";
import { useTheme } from "./theme-controller-model";
import { DARK_THEME, LIGHT_THEME } from "./theme-presets";
import { MAX_THEME_BYTES, THEME_TOKEN_NAMES, parseThemeDefinition, serializeThemeDefinition, type SpotThemeDefinition, type SpotThemeTokenName } from "./theme-schema";

function cloneTheme(theme: SpotThemeDefinition): SpotThemeDefinition {
  return { ...theme, tokens: { ...theme.tokens } };
}

function tokenLabel(token: SpotThemeTokenName): string {
  return token.replace(/([A-Z])/g, " $1").replace(/^./, (value) => value.toUpperCase());
}

function themeErrorFor(error: string | null, token: SpotThemeTokenName): string | null {
  if (!error) return null;
  return error.split("; ").find((part) => part.includes(`tokens.${token}`)) ?? null;
}

export function ThemeStudio() {
  const appearance = useTheme();
  const [draft, setDraft] = useState<SpotThemeDefinition>(cloneTheme(DARK_THEME));
  const [dirty, setDirty] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [exported, setExported] = useState<string | null>(null);
  const currentTheme = appearance.settings?.customTheme
    ?? (appearance.resolvedTheme === "light" ? LIGHT_THEME : DARK_THEME);
  const validationError = useMemo(() => {
    try {
      parseThemeDefinition(draft);
      return null;
    } catch (error) {
      return error instanceof Error ? error.message : "Theme validation failed.";
    }
  }, [draft]);

  useEffect(() => {
    if (!dirty && appearance.settings) setDraft(cloneTheme(currentTheme));
  }, [appearance.settings, currentTheme, dirty]);

  const updateDraft = (next: Partial<SpotThemeDefinition>) => {
    setDraft((current) => ({ ...current, ...next }));
    setDirty(true);
    setActionError(null);
  };
  const updateToken = (token: SpotThemeTokenName, value: string) => updateDraft({ tokens: { ...draft.tokens, [token]: value } });
  const validDraft = () => {
    try {
      return parseThemeDefinition(draft);
    } catch (error) {
      setActionError(error instanceof Error ? error.message : "Theme validation failed.");
      return null;
    }
  };
  const save = async () => {
    const valid = validDraft();
    if (!valid) return;
    try {
      await appearance.importCustomTheme(valid);
      setDirty(false);
    } catch (error) {
      setActionError(error instanceof Error ? error.message : "Theme could not be saved.");
    }
  };
  const importJson = async (file: File | undefined) => {
    if (!file) return;
    setActionError(null);
    try {
      if (file.size > MAX_THEME_BYTES) throw new Error(`Theme package exceeds the ${MAX_THEME_BYTES} byte limit.`);
      updateDraft(parseThemeDefinition(await file.text()));
    } catch (error) {
      setActionError(error instanceof Error ? error.message : "Theme JSON could not be imported.");
    }
  };
  const exportJson = () => {
    const valid = validDraft();
    if (!valid) return;
    const json = serializeThemeDefinition(valid);
    setExported(json);
    if (typeof Blob !== "undefined" && typeof URL.createObjectURL === "function") {
      const url = URL.createObjectURL(new Blob([json], { type: "application/json" }));
      const link = document.createElement("a");
      link.href = url;
      link.download = `${valid.name || "spotdiy-theme"}.json`;
      link.click();
      URL.revokeObjectURL(url);
    }
  };
  const reset = async () => {
    try {
      await appearance.resetCustomTheme();
      setDraft(cloneTheme(DARK_THEME));
      setDirty(false);
    } catch (error) {
      setActionError(error instanceof Error ? error.message : "Custom theme could not be reset.");
    }
  };

  return (
    <div className="theme-studio-editor">
      <section aria-label="Theme Studio controls" className="theme-studio-section">
        <div className="section-heading"><div><span className="eyebrow">THEME STUDIO</span><h2>Draft the atmosphere</h2></div><span className="section-note">Schema v1 · 15 tokens</span></div>
        <div className="theme-studio-starting-points"><span className="settings-muted-note">Start from</span><button className="button button-quiet button-small" onClick={() => { updateDraft(cloneTheme(DARK_THEME)); }} type="button">Clone Dark</button><button className="button button-quiet button-small" onClick={() => { updateDraft(cloneTheme(LIGHT_THEME)); }} type="button">Clone Light</button><button className="button button-quiet button-small" onClick={() => { updateDraft(cloneTheme(currentTheme)); }} type="button">Current Custom Theme</button></div>
        <div className="theme-studio-meta"><label><span>Theme name</span><input maxLength={80} onChange={(event) => updateDraft({ name: event.target.value })} value={draft.name} /></label><label><span>Base mode</span><select onChange={(event) => updateDraft({ baseMode: event.target.value as SpotThemeDefinition["baseMode"] })} value={draft.baseMode}><option value="dark">Dark</option><option value="light">Light</option></select></label></div>
        <div className="theme-token-grid">{THEME_TOKEN_NAMES.map((token) => { const error = themeErrorFor(validationError, token); const validColor = /^#[0-9a-fA-F]{6}$/.test(draft.tokens[token]); return <label className="theme-token-field" key={token}><span>{tokenLabel(token)}</span><div className="theme-token-inputs"><input aria-label={`${tokenLabel(token)} color`} onChange={(event) => updateToken(token, event.target.value)} type="color" value={validColor ? draft.tokens[token] : "#000000"} /><input aria-label={`${tokenLabel(token)} hex`} onChange={(event) => updateToken(token, event.target.value)} value={draft.tokens[token]} /></div>{error ? <small className="theme-token-error">{error.replace(/^tokens\.[^:]+:\s*/, "")}</small> : null}</label>; })}</div>
        {validationError ? <div className="library-inline-error" role="alert"><span>{validationError}</span></div> : null}
        {actionError || appearance.error ? <div className="library-inline-error" role="alert"><span>{actionError ?? appearance.error}</span></div> : null}
        <div className="theme-studio-actions"><label className="button button-secondary appearance-file-label">Import JSON<input accept="application/json,.json" aria-label="Import theme JSON" className="appearance-file-input" onChange={(event) => { void importJson(event.target.files?.[0]); event.currentTarget.value = ""; }} type="file" /></label><button className="button button-quiet" onClick={exportJson} type="button">Export JSON</button><button className="button button-primary" disabled={Boolean(validationError)} onClick={() => void save()} type="button">Save &amp; Activate</button><button className="button button-quiet" onClick={() => void reset()} type="button">Reset Custom Theme</button></div>
        {exported ? <textarea aria-label="Exported theme JSON" className="appearance-export" readOnly value={exported} /> : null}
      </section>
      <LayoutWorkspace />
    </div>
  );
}
