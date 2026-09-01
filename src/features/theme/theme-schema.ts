import { z, ZodError } from "zod";

export const THEME_SCHEMA_VERSION = 1 as const;
export const MAX_THEME_BYTES = 64 * 1024;

export const THEME_TOKEN_NAMES = [
  "background",
  "surface",
  "surfaceRaised",
  "surfaceSoft",
  "text",
  "textMuted",
  "textSubtle",
  "border",
  "borderStrong",
  "accent",
  "accentContrast",
  "success",
  "warning",
  "danger",
  "info",
] as const;

export type SpotThemeTokenName = (typeof THEME_TOKEN_NAMES)[number];

export type SpotThemeTokens = Record<SpotThemeTokenName, string>;

export interface SpotThemeDefinition {
  schemaVersion: typeof THEME_SCHEMA_VERSION;
  name: string;
  baseMode: "dark" | "light";
  tokens: SpotThemeTokens;
}

const hexColorSchema = z.string().regex(/^#[0-9a-fA-F]{6}$/, "must be a #RRGGBB color");

export const spotThemeTokensSchema = z.object({
  background: hexColorSchema,
  surface: hexColorSchema,
  surfaceRaised: hexColorSchema,
  surfaceSoft: hexColorSchema,
  text: hexColorSchema,
  textMuted: hexColorSchema,
  textSubtle: hexColorSchema,
  border: hexColorSchema,
  borderStrong: hexColorSchema,
  accent: hexColorSchema,
  accentContrast: hexColorSchema,
  success: hexColorSchema,
  warning: hexColorSchema,
  danger: hexColorSchema,
  info: hexColorSchema,
}).strict();

const rawSpotThemeDefinitionSchema = z.object({
  schemaVersion: z.literal(THEME_SCHEMA_VERSION),
  name: z.string()
    .trim()
    .min(1, "must not be empty")
    .refine((value) => Array.from(value).length <= 80, "must be at most 80 Unicode scalar values"),
  baseMode: z.enum(["dark", "light"]),
  tokens: spotThemeTokensSchema,
}).strict();

function colorToRgb(color: string): [number, number, number] | null {
  if (!/^#[0-9a-fA-F]{6}$/.test(color)) {
    return null;
  }
  return [
    Number.parseInt(color.slice(1, 3), 16),
    Number.parseInt(color.slice(3, 5), 16),
    Number.parseInt(color.slice(5, 7), 16),
  ];
}

function relativeLuminance(color: string): number | null {
  const rgb = colorToRgb(color);
  if (!rgb) {
    return null;
  }
  const channels = rgb.map((channel) => {
    const value = channel / 255;
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  });
  return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722;
}

export function contrastRatio(foreground: string, background: string): number | null {
  const foregroundLuminance = relativeLuminance(foreground);
  const backgroundLuminance = relativeLuminance(background);
  if (foregroundLuminance === null || backgroundLuminance === null) {
    return null;
  }
  const lighter = Math.max(foregroundLuminance, backgroundLuminance);
  const darker = Math.min(foregroundLuminance, backgroundLuminance);
  return (lighter + 0.05) / (darker + 0.05);
}

const contrastRequirements = [
  ["text", "background", 4.5],
  ["text", "surface", 4.5],
  ["textMuted", "background", 4.5],
  ["textMuted", "surface", 4.5],
  ["accent", "accentContrast", 4.5],
  ["accent", "background", 3],
  ["accent", "surface", 3],
] as const satisfies ReadonlyArray<readonly [SpotThemeTokenName, SpotThemeTokenName, number]>;

export const spotThemeDefinitionSchema = rawSpotThemeDefinitionSchema.superRefine((theme, context) => {
  for (const [foregroundName, backgroundName, minimum] of contrastRequirements) {
    const ratio = contrastRatio(theme.tokens[foregroundName], theme.tokens[backgroundName]);
    if (ratio !== null && ratio < minimum) {
      context.addIssue({
        code: "custom",
        path: ["tokens", foregroundName],
        message: `${foregroundName}/${backgroundName} contrast is ${ratio.toFixed(2)}:1; minimum is ${minimum}:1`,
      });
    }
  }
});

function utf8ByteLength(value: string): number {
  if (typeof TextEncoder !== "undefined") {
    return new TextEncoder().encode(value).byteLength;
  }
  return value.length;
}

function formatValidationError(error: unknown): string {
  if (error instanceof ZodError) {
    return error.issues
      .map((issue) => `${issue.path.length > 0 ? issue.path.join(".") : "theme"}: ${issue.message}`)
      .join("; ");
  }
  if (error instanceof Error && error.message) {
    return error.message;
  }
  return "Theme definition is invalid.";
}

export function parseThemeDefinition(json: unknown): SpotThemeDefinition {
  let input = json;
  if (typeof json === "string") {
    if (utf8ByteLength(json) > MAX_THEME_BYTES) {
      throw new Error(`Theme package exceeds the ${MAX_THEME_BYTES} byte limit.`);
    }
    try {
      input = JSON.parse(json) as unknown;
    } catch (error) {
      throw new Error(`Theme JSON could not be parsed: ${formatValidationError(error)}`, { cause: error });
    }
  }

  try {
    const theme = spotThemeDefinitionSchema.parse(input) as SpotThemeDefinition;
    const serialized = JSON.stringify(theme);
    if (utf8ByteLength(serialized) > MAX_THEME_BYTES) {
      throw new Error(`Theme package exceeds the ${MAX_THEME_BYTES} byte limit.`);
    }
    return theme;
  } catch (error) {
    if (error instanceof Error && error.message.includes("byte limit")) {
      throw error;
    }
    throw new Error(`Theme validation failed: ${formatValidationError(error)}`, { cause: error });
  }
}

export function serializeThemeDefinition(theme: unknown): string {
  const validated = parseThemeDefinition(theme);
  const serialized = JSON.stringify(validated, null, 2);
  if (utf8ByteLength(serialized) > MAX_THEME_BYTES) {
    throw new Error(`Theme package exceeds the ${MAX_THEME_BYTES} byte limit.`);
  }
  return serialized;
}

export function themeCssVariables(theme: SpotThemeDefinition): Record<string, string> {
  const validated = parseThemeDefinition(theme);
  return {
    "--color-bg": validated.tokens.background,
    "--color-surface": validated.tokens.surface,
    "--color-surface-raised": validated.tokens.surfaceRaised,
    "--color-surface-soft": validated.tokens.surfaceSoft,
    "--color-text": validated.tokens.text,
    "--color-text-muted": validated.tokens.textMuted,
    "--color-text-subtle": validated.tokens.textSubtle,
    "--color-border": validated.tokens.border,
    "--color-border-strong": validated.tokens.borderStrong,
    "--color-accent": validated.tokens.accent,
    "--color-accent-contrast": validated.tokens.accentContrast,
    "--color-success": validated.tokens.success,
    "--color-warning": validated.tokens.warning,
    "--color-danger": validated.tokens.danger,
    "--color-info": validated.tokens.info,
  };
}
