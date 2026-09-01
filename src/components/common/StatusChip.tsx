import type { HTMLAttributes, ReactNode } from "react";

export type StatusChipState = "neutral" | "success" | "warning" | "danger" | "info" | "accent";

export interface StatusChipProps extends HTMLAttributes<HTMLSpanElement> {
  status?: StatusChipState;
  children: ReactNode;
}

export function StatusChip({ children, className = "", status = "neutral", ...props }: StatusChipProps) {
  return (
    <span {...props} className={`status-chip status-chip-${status}${className ? ` ${className}` : ""}`} data-status={status} role={props.role ?? "status"}>
      {children}
    </span>
  );
}
