import type { OverlayKind } from "../../types/domain";
import { EdgeOverlay } from "./EdgeOverlay";
import { GamingOverlay } from "./GamingOverlay";
import { LyricsOverlay } from "./LyricsOverlay";
import { MiniOverlay } from "./MiniOverlay";

export function OverlayRoot({ kind }: { kind: OverlayKind }) {
  switch (kind) {
    case "mini":
      return <MiniOverlay />;
    case "edge":
      return <EdgeOverlay />;
    case "lyrics":
      return <LyricsOverlay />;
    case "gaming":
      return <GamingOverlay />;
  }
}
