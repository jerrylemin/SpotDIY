import type { ReactNode } from "react";

import { SpotIcon, type SpotIconName } from "../icons/SpotIcon";

interface EmptyStateProps {
  icon: SpotIconName;
  eyebrow: string;
  title: string;
  description: string;
  action?: ReactNode;
}

export function EmptyState({ icon, eyebrow, title, description, action }: EmptyStateProps) {
  return (
    <section className="empty-state" aria-label={title}>
      <div className="empty-state-icon">
        <SpotIcon name={icon} size={23} />
      </div>
      <span className="eyebrow">{eyebrow}</span>
      <h2>{title}</h2>
      <p>{description}</p>
      {action ? <div className="empty-state-action">{action}</div> : null}
    </section>
  );
}
