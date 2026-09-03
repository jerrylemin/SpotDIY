import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("Tauri CSP", () => {
  it("does not grant frontend arbitrary HTTPS connections", () => {
    const config = JSON.parse(
      readFileSync(resolve(process.cwd(), "src-tauri/tauri.conf.json"), "utf8"),
    ) as { app: { security: { csp: string } } };

    const connectSource = config.app.security.csp
      .split(";")
      .find((directive) => directive.trim().startsWith("connect-src"));

    expect(connectSource).toBeDefined();
    expect(connectSource).not.toMatch(/\bhttps:/);
  });
});
