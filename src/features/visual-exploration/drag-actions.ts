export type VisualDropAction = "play-next" | "queue";

export interface VisualDropResult {
  trackId: string;
  action: VisualDropAction;
}

export function resolveVisualDrop(trackId: string | null | undefined, action: string | null | undefined): VisualDropResult | null {
  if (!trackId || action !== "play-next" && action !== "queue") return null;
  return { trackId, action };
}
