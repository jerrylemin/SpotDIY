import { forwardRef, type HTMLAttributes } from "react";

export const Surface = forwardRef<HTMLDivElement, HTMLAttributes<HTMLDivElement>>(function Surface({ className = "", ...props }, ref) {
  return <div {...props} className={`surface${className ? ` ${className}` : ""}`} ref={ref} />;
});
