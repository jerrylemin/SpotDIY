import { forwardRef, type ButtonHTMLAttributes } from "react";

export type ButtonVariant = "primary" | "secondary" | "quiet" | "danger";
export type ButtonSize = "sm" | "md";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button({ className = "", size = "md", variant = "secondary", ...props }, ref) {
  return (
    <button
      {...props}
      className={`button button-${variant}${size === "sm" ? " button-small" : ""}${className ? ` ${className}` : ""}`}
      ref={ref}
    />
  );
});
