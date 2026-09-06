import type { OverlayKind } from "../../types/domain";
import { MiniOverlay } from "./MiniOverlay";

export function OverlayRoot({ kind }: { kind: OverlayKind }) {
  switch (kind) {
    case "mini":
      return <MiniOverlay />;
  }
}
