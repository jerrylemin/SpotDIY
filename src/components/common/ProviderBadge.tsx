import type { ProviderKind } from "../../types/domain";

import { providerLabel } from "../../services/ipc";

interface ProviderBadgeProps {
  kind: ProviderKind;
  subdued?: boolean;
  title?: string;
}

export function ProviderBadge({ kind, subdued = false, title }: ProviderBadgeProps) {
  const label = kind === "local" ? "Local" : kind === "youtube" ? "YouTube" : kind === "soundcloud" ? "SoundCloud" : "Spotify";
  return <span aria-label={label} className={`provider-badge provider-${kind}${subdued ? " provider-badge-subdued" : ""}`} title={title ?? label}>{providerLabel(kind)}</span>;
}
