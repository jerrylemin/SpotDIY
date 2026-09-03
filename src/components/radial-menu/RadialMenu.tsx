import { useEffect, useRef } from "react";

import { ContextActionMenu } from "../common/ContextActionMenu";
import { SpotIcon } from "../icons/SpotIcon";
import { visibleRadialActions, type RadialAction } from "./radial-actions";

interface RadialMenuProps {
  actions: readonly RadialAction[];
  anchor: { left: number; top: number };
  onClose: () => void;
  open: boolean;
}

export function RadialMenu({ actions, anchor, onClose, open }: RadialMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const { visible, more } = visibleRadialActions(actions);

  useEffect(() => {
    if (!open) return undefined;
    menuRef.current?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose, open]);

  if (!open) return null;
  const radius = 82;
  return (
    <div
      aria-label="Radial track actions"
      className="radial-menu"
      onMouseDown={(event) => event.stopPropagation()}
      ref={menuRef}
      role="menu"
      style={{ left: anchor.left, top: anchor.top }}
    >
      <button aria-label="Close radial actions" className="radial-center" onClick={onClose} type="button"><SpotIcon name="close" size={16} /></button>
      {visible.map((action, index) => {
        const angle = (index / Math.max(1, visible.length)) * Math.PI * 2 - Math.PI / 2;
        return (
          <button
            aria-disabled={action.disabled || undefined}
            className="radial-action"
            disabled={action.disabled}
            key={action.id}
            onClick={() => { onClose(); action.onSelect(); }}
            role="menuitem"
            style={{ transform: `translate(${Math.cos(angle) * radius}px, ${Math.sin(angle) * radius}px)` }}
            title={action.disabled ? action.disabledReason : action.label}
            type="button"
          >
            <span>{action.label}</span>
          </button>
        );
      })}
      {more.length > 0 ? (
        <ContextActionMenu actions={more} className="radial-more-wrap" label="More radial actions" menuLabel="More radial actions">
          <button className="radial-action radial-more" onClick={(event) => event.stopPropagation()} type="button"><SpotIcon name="more" size={15} /><span>More…</span></button>
        </ContextActionMenu>
      ) : null}
    </div>
  );
}
