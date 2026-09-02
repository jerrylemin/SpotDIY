import { describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import {
  IpcError,
  getAppStatus,
  getSettingsSnapshot,
  getWindowsIntegrationSnapshot,
  parseWindowsIntegrationSnapshot,
  resetGlobalShortcuts,
  providerLabel,
  setSetting,
  toggleOverlay,
  updateGlobalShortcut,
} from "../src/services/ipc";

describe("provider labels", () => {
  it("keeps compact badges distinct from provider names", () => {
    expect(providerLabel("local")).toBe("LOCAL");
    expect(providerLabel("youtube")).toBe("YT");
    expect(providerLabel("soundcloud")).toBe("SC");
    expect(providerLabel("spotify")).toBe("SP");
  });
});

describe("settings IPC contract", () => {
  it("provides typed browser-preview defaults", async () => {
    await expect(getSettingsSnapshot()).resolves.toEqual({
      theme: "dark",
      layoutProfile: "comfortable",
      customTheme: null,
      downloadsDirectory: null,
      sourcePreferenceOrder: ["local", "soundcloud", "youtube", "spotify"],
      firstRun: true,
      storageMode: "standard",
      windowsIntegration: { smtcEnabled: true, globalShortcutsEnabled: false },
      globalShortcuts: [
        { action: "playPause", accelerator: "Ctrl+Alt+Space", enabled: true },
        { action: "next", accelerator: "Ctrl+Alt+Right", enabled: true },
        { action: "previous", accelerator: "Ctrl+Alt+Left", enabled: true },
        { action: "volumeUp", accelerator: "Ctrl+Alt+Up", enabled: true },
        { action: "volumeDown", accelerator: "Ctrl+Alt+Down", enabled: true },
        { action: "showHideMain", accelerator: "Ctrl+Alt+S", enabled: true },
        { action: "toggleMiniOverlay", accelerator: "Ctrl+Alt+M", enabled: true },
        { action: "toggleLyricsOverlay", accelerator: "Ctrl+Alt+L", enabled: true },
        { action: "toggleGamingOverlay", accelerator: "Ctrl+Alt+G", enabled: true },
      ],
      outputProfiles: [],
    });
  });

  it("keeps appearance writes inside the browser-preview settings adapter", async () => {
    await expect(setSetting({ key: "layoutProfile", value: "dense" })).resolves.toMatchObject({
      theme: "dark",
      layoutProfile: "dense",
    });
    await expect(setSetting({ key: "theme", value: "light" })).resolves.toMatchObject({ theme: "light" });
  });

  it("keeps Windows integration DTOs strict and browser overlay state typed", async () => {
    const snapshot = await getWindowsIntegrationSnapshot();
    expect(snapshot.platformSupported).toBe(false);
    expect(snapshot.overlays).toEqual([
      { kind: "mini", status: "closed", detail: null },
      { kind: "edge", status: "closed", detail: null },
      { kind: "lyrics", status: "closed", detail: null },
      { kind: "gaming", status: "closed", detail: null },
    ]);
    const opened = await toggleOverlay("mini");
    expect(opened.overlays.find((overlay) => overlay.kind === "mini")?.status).toBe("open");
    expect(() => parseWindowsIntegrationSnapshot({ ...snapshot, unexpected: true })).toThrow();
  });

  it("resets browser-preview shortcut edits to the frozen defaults", async () => {
    await updateGlobalShortcut({ action: "playPause", accelerator: "Ctrl+Shift+P", enabled: true });
    expect((await getWindowsIntegrationSnapshot()).shortcutStatuses.find((item) => item.action === "playPause")?.accelerator).toBe("Ctrl+Shift+P");
    const reset = await resetGlobalShortcuts();
    expect(reset.shortcutStatuses.find((item) => item.action === "playPause")?.accelerator).toBe("Ctrl+Alt+Space");
  });

  it("wraps malformed native status responses", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    invokeMock.mockResolvedValueOnce({ runtime: "tauri" });

    try {
      await expect(getAppStatus()).rejects.toBeInstanceOf(IpcError);
    } finally {
      Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
      invokeMock.mockReset();
    }
  });

  it("wraps native setting persistence failures", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    invokeMock.mockRejectedValueOnce(new Error("database failure"));

    try {
      await expect(setSetting({ key: "theme", value: "light" })).rejects.toBeInstanceOf(IpcError);
    } finally {
      Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
      invokeMock.mockReset();
    }
  });
});
