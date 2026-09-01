import type { LayoutProfile } from "../../types/domain";

export const LAYOUT_PROFILES = ["comfortable", "compact", "dense"] as const satisfies readonly LayoutProfile[];

export const LAYOUT_PROFILE_LABELS: Record<LayoutProfile, string> = {
  comfortable: "Comfortable",
  compact: "Compact",
  dense: "Dense",
};

export function isLayoutProfile(value: unknown): value is LayoutProfile {
  return typeof value === "string" && (LAYOUT_PROFILES as readonly string[]).includes(value);
}
