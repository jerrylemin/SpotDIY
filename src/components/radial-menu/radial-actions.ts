export interface RadialAction {
  id: string;
  label: string;
  onSelect: () => void;
  disabled?: boolean;
  disabledReason?: string;
}

export function visibleRadialActions(actions: readonly RadialAction[], maxVisible = 8): { visible: RadialAction[]; more: RadialAction[] } {
  const visibleCount = actions.length > maxVisible ? Math.max(0, maxVisible - 1) : maxVisible;
  return { visible: actions.slice(0, visibleCount), more: actions.slice(visibleCount) };
}
