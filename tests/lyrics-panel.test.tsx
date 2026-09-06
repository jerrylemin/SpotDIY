import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { LyricsPanel } from "../src/components/lyrics/LyricsPanel";
import type { LyricsDocument, TrackId } from "../src/types/domain";

afterEach(cleanup);

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
  it("pauses follow on manual scrolling and resumes without scrolling the page", () => {
    const scrollTo = vi.fn();
    const { container, rerender } = render(<LyricsPanel document={lyrics} onSeek={vi.fn()} positionMs={1_000} />);
    const viewport = screen.getByLabelText("Timed lyrics");
    Object.defineProperty(viewport, "scrollTo", { value: scrollTo, configurable: true });
    fireEvent.wheel(viewport);
    rerender(<LyricsPanel document={lyrics} onSeek={vi.fn()} positionMs={2_100} />);
    expect(scrollTo).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Back to current line ↓" }));
    expect(scrollTo).toHaveBeenCalledTimes(1);
    expect(container.querySelector(".lyrics-cue-past")).toHaveTextContent("hello");
    expect(screen.queryByRole("button", { name: "Back to current line ↓" })).not.toBeInTheDocument();
  });

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
