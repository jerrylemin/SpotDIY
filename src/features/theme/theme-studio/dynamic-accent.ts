import { contrastRatio } from "../theme-schema";

function rgbToHex(red: number, green: number, blue: number): string {
  return `#${[red, green, blue].map((value) => Math.round(value).toString(16).padStart(2, "0")).join("").toUpperCase()}`;
}

function validAccent(accent: string, background: string, surface: string): { accent: string; accentContrast: "#000000" | "#FFFFFF" } | null {
  const black = contrastRatio(accent, "#000000") ?? 0;
  const white = contrastRatio(accent, "#FFFFFF") ?? 0;
  const accentContrast = black >= white ? "#000000" : "#FFFFFF";
  if ((contrastRatio(accent, background) ?? 0) < 3 || (contrastRatio(accent, surface) ?? 0) < 3 || (contrastRatio(accent, accentContrast) ?? 0) < 4.5) return null;
  return { accent, accentContrast };
}

export function sampleAccentFromPixels(pixels: Uint8ClampedArray, background: string, surface: string): { accent: string; accentContrast: "#000000" | "#FFFFFF" } | null {
  const sampleCount = Math.min(4_096, Math.floor(pixels.length / 4));
  if (sampleCount === 0) return null;
  let red = 0;
  let green = 0;
  let blue = 0;
  let count = 0;
  for (let index = 0; index < sampleCount; index += 1) {
    const offset = index * 4;
    if (pixels[offset + 3] < 16) continue;
    red += pixels[offset];
    green += pixels[offset + 1];
    blue += pixels[offset + 2];
    count += 1;
  }
  if (count === 0) return null;
  const source: [number, number, number] = [red / count, green / count, blue / count];
  for (let iteration = 0; iteration <= 12; iteration += 1) {
    const factor = iteration === 0 ? 1 : iteration % 2 === 0 ? 1 + iteration * 0.08 : 1 - iteration * 0.06;
    const candidate = rgbToHex(...source.map((channel) => Math.min(255, Math.max(0, channel * factor)) as number) as [number, number, number]);
    const valid = validAccent(candidate, background, surface);
    if (valid) return valid;
  }
  return null;
}
