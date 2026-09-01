import type { HTMLAttributes, ReactNode } from "react";

import { SpotIcon, type SpotIconName } from "../icons/SpotIcon";

export interface EmptyStateProps extends HTMLAttributes<HTMLElement> {
  icon: SpotIconName;
  eyebrow: string;
  title: string;
  description: string;
  action?: ReactNode;
}

export function EmptyState({ action, className = "", description, eyebrow, icon, title, ...props }: EmptyStateProps) {
  return (
    <section {...props} className={`empty-state${className ? ` ${className}` : ""}`} aria-label={props["aria-label"] ?? title}>
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
