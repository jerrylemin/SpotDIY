import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { LyricsPanel } from "../src/components/lyrics/LyricsPanel";
import type { LyricsDocument, TrackId } from "../src/types/domain";

const lyrics: LyricsDocument = {
  trackId: "track-panel" as TrackId,
  source: "manual",
  syncKind: "timed",
  plainText: "hello world\nnext line",
  cues: [
    {
      startMs: 1_000,
      lines: ["hello world"],
      words: [
        { startMs: 1_000, text: "hello" },
        { startMs: 1_500, text: "world" },
      ],
    },
    { startMs: 2_000, lines: ["next line"], words: [] },
  ],
  instrumental: false,
  editable: true,
  attribution: null,
};

describe("LyricsPanel", () => {
  it("highlights the active word, renders cue progress, and seeks from a cue click", () => {
    const onSeek = vi.fn();
    const { container } = render(<LyricsPanel document={lyrics} durationMs={5_000} onSeek={onSeek} positionMs={1_750} />);

    expect(screen.getByText("hello")).toHaveClass("lyrics-word-active");
    expect(screen.getByText("world")).toHaveClass("lyrics-word-active");
    expect(screen.getAllByRole("button")[0]).toHaveAttribute("aria-current", "true");
    expect(container.querySelectorAll(".lyrics-cue-progress")).toHaveLength(2);

    fireEvent.click(screen.getByText("next line"));
    expect(onSeek).toHaveBeenCalledWith(2_000);
  });

  it("applies the configured offset before selecting a cue", () => {
    const onSeek = vi.fn();
    const { container } = render(
      <LyricsPanel document={lyrics} lyricOffsetMs={300} onSeek={onSeek} positionMs={800} />,
    );

    expect(screen.getAllByRole("button")[0]).toHaveAttribute("aria-current", "true");
    fireEvent.click(container.querySelectorAll<HTMLButtonElement>(".lyrics-cue")[1]);
    expect(onSeek).toHaveBeenCalledWith(1_700);
  });
});
