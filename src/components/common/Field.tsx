import { forwardRef, useId, type InputHTMLAttributes } from "react";

export interface FieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "id"> {
  label: string;
  id?: string;
  hint?: string;
  error?: string;
}

export const Field = forwardRef<HTMLInputElement, FieldProps>(function Field({ error, hint, id: providedId, label, ...props }, ref) {
  const generatedId = useId();
  const id = providedId ?? generatedId;
  const hintId = hint ? `${id}-hint` : undefined;
  const errorId = error ? `${id}-error` : undefined;
  const describedBy = [props["aria-describedby"], hintId, errorId].filter(Boolean).join(" ") || undefined;

  return (
    <label className="field" htmlFor={id}>
      <span className="field-label">{label}</span>
      <input
        {...props}
        aria-describedby={describedBy}
        aria-invalid={error ? true : props["aria-invalid"]}
        className={`field-input${props.className ? ` ${props.className}` : ""}`}
        id={id}
        ref={ref}
      />
      {hint ? <span className="field-hint" id={hintId}>{hint}</span> : null}
      {error ? <span className="field-error" id={errorId} role="alert">{error}</span> : null}
    </label>
  );
});
