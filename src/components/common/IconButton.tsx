import { forwardRef, type ButtonHTMLAttributes } from "react";

export interface IconButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "aria-label"> {
  "aria-label": string;
}

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton({ className = "", ...props }, ref) {
  return <button {...props} className={`icon-button${className ? ` ${className}` : ""}`} ref={ref} />;
});
