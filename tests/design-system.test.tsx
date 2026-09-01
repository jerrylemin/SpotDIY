import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { Button } from "../src/components/common/Button";
import { ContextActionMenu } from "../src/components/common/ContextActionMenu";
import { IconButton } from "../src/components/common/IconButton";
import { SegmentedControl } from "../src/components/common/SegmentedControl";
import { StatusChip } from "../src/components/common/StatusChip";
import { IconGallery } from "../src/components/icons/IconGallery";
import { spotIconNames } from "../src/components/icons/SpotIcon";
import { InspectorPanel } from "../src/components/inspector/InspectorPanel";
import { ThemeController, resolveTheme, useTheme } from "../src/features/theme/theme-controller";
import { DARK_THEME } from "../src/features/theme/theme-presets";
import { getSettingsSnapshot, setSetting } from "../src/services/ipc";

afterEach(cleanup);

function QueryWrapper({ children }: { children: React.ReactNode }) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
}

describe("accessible design-system primitives", () => {
  it("preserves button semantics and requires an icon button name", () => {
    render(<><Button type="submit" variant="primary">Save</Button><IconButton aria-label="More actions"><span>+</span></IconButton></>);
    expect(screen.getByRole("button", { name: "Save" })).toHaveClass("button-primary");
    expect(screen.getByRole("button", { name: "More actions" })).toHaveClass("icon-button");
  });

  it("exposes visible status text and keyboard segmented control behavior", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<><StatusChip status="warning">Needs attention</StatusChip><SegmentedControl label="Density" onChange={onChange} options={[{ value: "one", label: "One" }, { value: "two", label: "Two" }, { value: "three", label: "Three" }]} value="one" /></>);
    expect(screen.getByText("Needs attention")).toBeVisible();
    await user.tab();
    await user.keyboard("{ArrowRight}");
    expect(onChange).toHaveBeenCalledWith("two");
  });

  it("opens context actions from right click and Shift+F10, navigates, restores focus, and explains disabled items", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    render(<ContextActionMenu actions={[{ id: "open", label: "Open", onSelect }, { id: "disabled", label: "Unavailable", onSelect, disabled: true, disabledReason: "Native app only" }]} label="Demo track"><span>Demo track</span></ContextActionMenu>);
    const trigger = screen.getByRole("group", { name: "Demo track" });
    fireEvent.contextMenu(trigger, { clientX: 40, clientY: 40 });
    expect(await screen.findByRole("menu", { name: "Context actions" })).toBeVisible();
    expect(screen.getByRole("menuitem", { name: /UnavailableNative app only/ })).toBeDisabled();
    expect(screen.getByText("Native app only")).toBeVisible();
    await user.keyboard("{ArrowDown}");
    await user.keyboard("{Escape}");
    expect(trigger).toHaveFocus();

    fireEvent.keyDown(trigger, { key: "F10", shiftKey: true });
    expect(await screen.findByRole("menu")).toBeVisible();
    await user.keyboard("{Enter}");
    expect(onSelect).toHaveBeenCalledTimes(1);
  });

  it("closes the inspector with Escape", async () => {
    const onClose = vi.fn();
    render(<InspectorPanel onClose={onClose} sections={[{ id: "one", title: "One", content: <p>Details</p> }]} title="Inspector" />);
    expect(screen.getByRole("dialog", { name: "Inspector" })).toBeVisible();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("renders every declared SpotIcon without missing path data", () => {
    render(<IconGallery />);
    for (const name of spotIconNames) {
      expect(screen.getByText(name)).toBeVisible();
    }
    for (const path of document.querySelectorAll(".icon-gallery-item path")) {
      expect(path.getAttribute("d")).toBeTruthy();
      expect(path.getAttribute("d")).not.toContain("undefined");
      expect(path.getAttribute("d")).not.toContain("NaN");
    }
  });
});

function ThemeReadout() {
  const theme = useTheme();
  return <span data-testid="theme-readout">{theme.theme}:{theme.resolvedSystemTheme}:{theme.settings?.layoutProfile ?? "loading"}</span>;
}

describe("theme controller", () => {
  let mediaListener: ((event: MediaQueryListEvent) => void) | undefined;
  const removeEventListener = vi.fn();

  beforeEach(() => {
    mediaListener = undefined;
    removeEventListener.mockReset();
    vi.stubGlobal("matchMedia", vi.fn().mockImplementation(() => ({
      matches: false,
      media: "(prefers-color-scheme: dark)",
      addEventListener: vi.fn((_type: string, listener: (event: MediaQueryListEvent) => void) => { mediaListener = listener; }),
      removeEventListener,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      onchange: null,
      dispatchEvent: vi.fn(),
    })));
    document.documentElement.removeAttribute("data-theme");
    document.documentElement.removeAttribute("data-layout");
    for (const name of ["--color-bg", "--color-accent"]) document.documentElement.style.removeProperty(name);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("applies dark defaults and layout attributes", async () => {
    render(<QueryWrapper><ThemeController><ThemeReadout /></ThemeController></QueryWrapper>);
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe("dark"));
    expect(document.documentElement.dataset.layout).toBe("comfortable");
  });

  it("keeps System stored while following an OS preference change", async () => {
    await setSetting({ key: "theme", value: "system" });
    render(<QueryWrapper><ThemeController><ThemeReadout /></ThemeController></QueryWrapper>);
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe("light"));
    expect(screen.getByTestId("theme-readout")).toHaveTextContent("system:light");
    mediaListener?.({ matches: true } as MediaQueryListEvent);
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe("dark"));
    expect(screen.getByTestId("theme-readout")).toHaveTextContent("system:dark");
    expect(removeEventListener).not.toHaveBeenCalled();
  });

  it("applies validated custom tokens and cleans up the listener", async () => {
    await setSetting({ key: "customTheme", value: DARK_THEME });
    await setSetting({ key: "theme", value: "custom" });
    const view = render(<QueryWrapper><ThemeController><ThemeReadout /></ThemeController></QueryWrapper>);
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe("custom"));
    expect(document.documentElement.style.getPropertyValue("--color-bg")).toBe(DARK_THEME.tokens.background);
    view.unmount();
    expect(removeEventListener).toHaveBeenCalled();
  });

  it("keeps the settings query shape available to the controller", async () => {
    await expect(getSettingsSnapshot()).resolves.toMatchObject({ layoutProfile: expect.any(String), customTheme: expect.anything() });
  });

  it("falls back to dark when a custom definition is invalid", () => {
    const invalidTheme = { ...DARK_THEME, tokens: { ...DARK_THEME.tokens, text: "#111111" } };
    expect(resolveTheme("custom", "light", invalidTheme)).toBe("dark");
  });
});
