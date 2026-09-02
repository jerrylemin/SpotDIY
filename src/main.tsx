import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "@tanstack/react-router";
import { MotionConfig } from "motion/react";

import { OverlayRoot } from "./components/overlay/OverlayRoot";
import { router } from "./routes";
import { ThemeController } from "./features/theme/theme-controller";
import { isTauriRuntime } from "./services/ipc";
import type { OverlayKind } from "./types/domain";
import "./styles/globals.css";
import "./styles/overlays.css";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false,
    },
  },
});

const overlayLabels = new Map<string, OverlayKind>([
  ["overlay-mini", "mini"],
  ["overlay-edge", "edge"],
  ["overlay-lyrics", "lyrics"],
  ["overlay-gaming", "gaming"],
]);

async function nativeWindowLabel(): Promise<string> {
  if (!isTauriRuntime()) {
    return "main";
  }
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    return getCurrentWindow().label;
  } catch {
    return "main";
  }
}

async function bootstrap() {
  const label = await nativeWindowLabel();
  const overlayKind = overlayLabels.get(label);
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <MotionConfig reducedMotion="user">
          <ThemeController>
            {overlayKind ? <OverlayRoot kind={overlayKind} /> : <RouterProvider router={router} />}
          </ThemeController>
        </MotionConfig>
      </QueryClientProvider>
    </StrictMode>,
  );
}

void bootstrap();
