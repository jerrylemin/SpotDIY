import { describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import {
  IpcError,
  getAppStatus,
  getSettingsSnapshot,
  providerLabel,
  setSetting,
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
    });
  });

  it("keeps appearance writes inside the browser-preview settings adapter", async () => {
    await expect(setSetting({ key: "layoutProfile", value: "dense" })).resolves.toMatchObject({
      theme: "dark",
      layoutProfile: "dense",
    });
    await expect(setSetting({ key: "theme", value: "light" })).resolves.toMatchObject({ theme: "light" });
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
