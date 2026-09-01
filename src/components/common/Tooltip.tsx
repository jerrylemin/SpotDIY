import type { HTMLAttributes, ReactNode } from "react";

export interface TooltipProps extends HTMLAttributes<HTMLSpanElement> {
  content: string;
  children: ReactNode;
}

export function Tooltip({ children, content, ...props }: TooltipProps) {
  return <span {...props} className={`tooltip${props.className ? ` ${props.className}` : ""}`} data-tooltip={content}>{children}</span>;
}
