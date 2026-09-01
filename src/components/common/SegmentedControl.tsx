import { useRef, type KeyboardEvent, type ReactNode } from "react";

export interface SegmentedControlOption<T extends string> {
  value: T;
  label: ReactNode;
  disabled?: boolean;
}

export interface SegmentedControlProps<T extends string> {
  value: T;
  options: readonly SegmentedControlOption<T>[];
  onChange: (value: T) => void;
  label: string;
  className?: string;
}

export function SegmentedControl<T extends string>({ className = "", label, onChange, options, value }: SegmentedControlProps<T>) {
  const buttonRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const enabledOptions = options.filter((option) => !option.disabled);

  function focusOption(option: SegmentedControlOption<T> | undefined) {
    if (!option) {
      return;
    }
    const index = options.indexOf(option);
    buttonRefs.current[index]?.focus();
    onChange(option.value);
  }

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const focusedIndex = buttonRefs.current.findIndex((button) => button === document.activeElement);
    const focusedOption = focusedIndex >= 0 ? options[focusedIndex] : undefined;
    const currentIndex = Math.max(0, enabledOptions.findIndex((option) => option.value === (focusedOption?.value ?? value)));
    let nextIndex: number | null = null;
    if (event.key === "ArrowRight" || event.key === "ArrowDown") {
      nextIndex = (currentIndex + 1) % enabledOptions.length;
    } else if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
      nextIndex = (currentIndex - 1 + enabledOptions.length) % enabledOptions.length;
    } else if (event.key === "Home") {
      nextIndex = 0;
    } else if (event.key === "End") {
      nextIndex = enabledOptions.length - 1;
    }
    if (nextIndex !== null && enabledOptions.length > 0) {
      event.preventDefault();
      focusOption(enabledOptions[nextIndex]);
    }
  }

  return (
    <div aria-label={label} className={`segmented-control${className ? ` ${className}` : ""}`} onKeyDown={handleKeyDown} role="radiogroup">
      {options.map((option, index) => (
        <button
          aria-checked={option.value === value}
          className="segmented-control-item"
          disabled={option.disabled}
          key={option.value}
          onClick={() => onChange(option.value)}
          ref={(element) => { buttonRefs.current[index] = element; }}
          role="radio"
          tabIndex={option.value === value ? 0 : -1}
          type="button"
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}
