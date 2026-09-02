import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";

const useBackupMock = vi.hoisted(() => vi.fn());

vi.mock("../src/hooks/useBackup", () => ({ useBackup: useBackupMock }));

import { BackupSection } from "../src/components/backup/BackupSection";

const preview = {
  importId: "11111111-1111-4111-8111-111111111111",
  archiveVersion: 1,
  appVersion: "0.1.0",
  databaseSchemaVersion: 8,
  sourceStorageMode: "standard" as const,
  entryCount: 1,
  includedAudioCount: 0,
  includedArtworkCount: 0,
  includedSidecarLyricsCount: 0,
  missing: {
    totalLocalReferences: 0,
    availableLocalReferences: 0,
    missingLocalReferences: 0,
    completedDownloadReferences: 0,
    missingDownloadOutputs: 0,
    firstMissing: [],
  },
  checksumValid: true,
  restoredAudioPlannedCount: 0,
};

function backupState(overrides: Partial<ReturnType<typeof useBackupMock>> = {}) {
  return {
    storage: {
      mode: "standard" as const,
      dataRoot: "C:\\Users\\Test\\AppData\\Local\\SpotDIY",
      databasePath: "C:\\Users\\Test\\AppData\\Local\\SpotDIY\\spotdiy.sqlite3",
      cacheRoot: "C:\\Users\\Test\\AppData\\Local\\SpotDIY\\cache",
      portableMarkerPresent: false,
      restartRequired: false,
      pendingImport: false,
      lastRollbackPath: null,
    },
    preview: null,
    loading: false,
    busy: false,
    error: null,
    clearError: vi.fn(),
    refresh: vi.fn(),
    exportBackup: vi.fn(),
    prepareImport: vi.fn(),
    commitImport: vi.fn(),
    cancelImport: vi.fn(),
    switchMode: vi.fn(),
    ...overrides,
  };
}

afterEach(() => {
  cleanup();
  useBackupMock.mockReset();
});

describe("BackupSection", () => {
  it("keeps export options and native actions explicit", () => {
    const state = backupState();
    useBackupMock.mockReturnValue(state);
    render(<BackupSection />);

    expect(screen.getByText("Standard active")).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: /sidecar lyrics/i })).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox", { name: /local audio/i }));
    expect(screen.getByRole("checkbox", { name: /sidecar lyrics/i })).toBeEnabled();
    fireEvent.click(screen.getByRole("button", { name: /export \.spotdiy/i }));
    expect(state.exportBackup).toHaveBeenCalledWith({
      includeLocalAudio: true,
      includeArtworkCache: false,
      includeSidecarLyrics: false,
    });
    fireEvent.click(screen.getByRole("button", { name: /prepare portable mode/i }));
    expect(state.switchMode).toHaveBeenCalledWith("portable");
  });

  it("requires an explicit confirmation for a staged import", () => {
    const state = backupState({ preview });
    useBackupMock.mockReturnValue(state);
    render(<BackupSection />);

    expect(screen.getByLabelText("SpotDIY import preview")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /confirm import/i }));
    expect(state.commitImport).toHaveBeenCalledWith(preview.importId);
    fireEvent.click(screen.getByRole("button", { name: /cancel import/i }));
    expect(state.cancelImport).toHaveBeenCalledWith(preview.importId);
  });
});
