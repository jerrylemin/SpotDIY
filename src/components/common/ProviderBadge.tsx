import type { ProviderKind } from "../../types/domain";

import { providerLabel } from "../../services/ipc";

interface ProviderBadgeProps {
  kind: ProviderKind;
  subdued?: boolean;
}

export function ProviderBadge({ kind, subdued = false }: ProviderBadgeProps) {
  return <span className={`provider-badge provider-${kind}${subdued ? " provider-badge-subdued" : ""}`}>{providerLabel(kind)}</span>;
}
