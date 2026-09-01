import { describe, expect, it } from "vitest";

import { DARK_THEME, LIGHT_THEME } from "./theme-presets";
import {
  parseThemeDefinition,
  serializeThemeDefinition,
  spotThemeDefinitionSchema,
} from "./theme-schema";

describe("SpotDIY theme definition", () => {
  it("accepts the built-in dark and light contracts", () => {
    expect(parseThemeDefinition(DARK_THEME)).toEqual(DARK_THEME);
    expect(parseThemeDefinition(LIGHT_THEME)).toEqual(LIGHT_THEME);
  });

  it("rejects an unsupported schema version, malformed color, unknown token, and low contrast", () => {
    expect(() => parseThemeDefinition({ ...DARK_THEME, schemaVersion: 2 })).toThrow(/schemaVersion/);
    expect(() => parseThemeDefinition({
      ...DARK_THEME,
      tokens: { ...DARK_THEME.tokens, accent: "rgb(1, 2, 3)" },
    })).toThrow(/accent/);
    expect(() => parseThemeDefinition({
      ...DARK_THEME,
      tokens: { ...DARK_THEME.tokens, extra: "#ffffff" },
    })).toThrow(/unrecognized|unknown|extra/i);
    expect(() => parseThemeDefinition({
      ...DARK_THEME,
      tokens: { ...DARK_THEME.tokens, text: "#111111", textMuted: "#121212" },
    })).toThrow(/contrast/);
  });

  it("trims the name and round trips validated JSON", () => {
    const input = { ...LIGHT_THEME, name: "  Quiet light  " };
    const parsed = parseThemeDefinition(input);
    const serialized = serializeThemeDefinition(parsed);

    expect(parsed.name).toBe("Quiet light");
    expect(parseThemeDefinition(serialized)).toEqual(parsed);
    expect(spotThemeDefinitionSchema.safeParse(parsed).success).toBe(true);
  });

  it("rejects a serialized package over 64 KiB", () => {
    expect(() => parseThemeDefinition(`${" ".repeat(64 * 1024)}${JSON.stringify(DARK_THEME)}`)).toThrow(/byte limit/);
  });
});
