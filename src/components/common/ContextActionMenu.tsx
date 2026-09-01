import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent, type MouseEvent, type ReactNode } from "react";

import { SpotIcon } from "../icons/SpotIcon";

export interface ContextAction {
  id: string;
  label: string;
  onSelect: () => void;
  disabled?: boolean;
  disabledReason?: string;
  danger?: boolean;
}

export interface ContextActionMenuProps {
  actions: readonly ContextAction[];
  children: ReactNode;
  label: string;
  menuLabel?: string;
  showMoreButton?: boolean;
  className?: string;
}

function menuItems(menu: HTMLDivElement | null): HTMLButtonElement[] {
  return menu ? Array.from(menu.querySelectorAll<HTMLButtonElement>("button:not(:disabled)")) : [];
}

export function ContextActionMenu({ actions, children, className = "", label, menuLabel = "Context actions", showMoreButton = true }: ContextActionMenuProps) {
  const triggerRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState({ left: 0, top: 0 });

  function openMenu(left: number, top: number) {
    const maxLeft = Math.max(0, window.innerWidth - 220);
    const maxTop = Math.max(0, window.innerHeight - Math.min(360, actions.length * 40 + 8));
    setPosition({ left: Math.min(Math.max(0, left), maxLeft), top: Math.min(Math.max(0, top), maxTop) });
    setOpen(true);
  }

  function openFromTrigger() {
    const rect = triggerRef.current?.getBoundingClientRect();
    openMenu(rect?.left ?? 0, (rect?.bottom ?? 0) + 4);
  }

  function closeMenu() {
    setOpen(false);
    triggerRef.current?.focus();
  }

  function handleContextMenu(event: MouseEvent<HTMLDivElement>) {
    event.preventDefault();
    openMenu(event.clientX, event.clientY);
  }

  function handleTriggerKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
      event.preventDefault();
      openFromTrigger();
    }
  }

  function handleMenuKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const items = menuItems(menuRef.current);
    if (event.key === "Escape") {
      event.preventDefault();
      closeMenu();
      return;
    }
    if (items.length === 0) {
      return;
    }
    const currentIndex = Math.max(0, items.indexOf(document.activeElement as HTMLButtonElement));
    let nextIndex: number | null = null;
    if (event.key === "ArrowDown") nextIndex = (currentIndex + 1) % items.length;
    if (event.key === "ArrowUp") nextIndex = (currentIndex - 1 + items.length) % items.length;
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = items.length - 1;
    if (nextIndex !== null) {
      event.preventDefault();
      items[nextIndex]?.focus();
    }
  }

  useEffect(() => {
    if (!open) {
      return undefined;
    }
    menuItems(menuRef.current)[0]?.focus();
    const onPointerDown = (event: PointerEvent) => {
      if (!menuRef.current?.contains(event.target as Node) && !triggerRef.current?.contains(event.target as Node)) {
        closeMenu();
      }
    };
    const onWindowKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        closeMenu();
      }
    };
    document.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("keydown", onWindowKeyDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("keydown", onWindowKeyDown);
    };
  }, [open]);

  const menuStyle: CSSProperties = { left: position.left, top: position.top };
  return (
    <div
      aria-expanded={open}
      aria-haspopup="menu"
      aria-label={label}
      className={`context-action-anchor${className ? ` ${className}` : ""}`}
      onContextMenu={handleContextMenu}
      onKeyDown={handleTriggerKeyDown}
      ref={triggerRef}
      role="group"
      tabIndex={0}
    >
      {children}
      {showMoreButton ? (
        <button aria-label={`More actions for ${label}`} className="context-action-trigger icon-button" onClick={openFromTrigger} type="button">
          <SpotIcon name="more" size={17} />
        </button>
      ) : null}
      {open ? (
        <div aria-label={menuLabel} className="context-action-menu" onKeyDown={handleMenuKeyDown} ref={menuRef} role="menu" style={menuStyle}>
          {actions.map((action) => (
            <button
              aria-disabled={action.disabled || undefined}
              className={`context-action-item${action.danger ? " context-action-item-danger" : ""}`}
              data-highlighted={false}
              disabled={action.disabled}
              key={action.id}
              onClick={() => { closeMenu(); action.onSelect(); }}
              role="menuitem"
              title={action.disabled ? action.disabledReason : undefined}
              type="button"
            >
              <span>{action.label}</span>
              {action.disabled && action.disabledReason ? <span className="context-action-item-reason">{action.disabledReason}</span> : null}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}
