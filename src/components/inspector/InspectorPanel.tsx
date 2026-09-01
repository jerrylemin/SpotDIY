import { useEffect, useId, useRef, type ReactNode } from "react";

import { IconButton } from "../common/IconButton";
import { SpotIcon } from "../icons/SpotIcon";

export interface InspectorSection {
  id: string;
  title: string;
  content: ReactNode;
}

export interface InspectorPanelProps {
  title: string;
  subtitle?: string;
  sections: readonly InspectorSection[];
  onClose: () => void;
  open?: boolean;
}

export function InspectorPanel({ onClose, open = true, sections, subtitle, title }: InspectorPanelProps) {
  const panelRef = useRef<HTMLElement>(null);
  const titleId = useId();

  useEffect(() => {
    if (!open) {
      return undefined;
    }
    panelRef.current?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose, open]);

  if (!open) {
    return null;
  }

  return (
    <aside aria-labelledby={titleId} aria-modal="true" className="inspector-panel" ref={panelRef} role="dialog" tabIndex={-1}>
      <header className="inspector-header">
        <div>
          <h2 id={titleId}>{title}</h2>
          {subtitle ? <p>{subtitle}</p> : null}
        </div>
        <IconButton aria-label="Close inspector" className="inspector-close" onClick={onClose}>
          <SpotIcon name="close" size={17} />
        </IconButton>
      </header>
      <div className="inspector-body">
        {sections.map((section) => (
          <section className="inspector-section" id={section.id} key={section.id}>
            <h3>{section.title}</h3>
            {section.content}
          </section>
        ))}
      </div>
    </aside>
  );
}
