interface StatusDotProps {
  active?: boolean;
  label: string;
}

export function StatusDot({ active = true, label }: StatusDotProps) {
  return (
    <span className="status-dot-wrap">
      <span className={`status-dot${active ? " status-dot-active" : ""}`} aria-hidden="true" />
      <span>{label}</span>
    </span>
  );
}
