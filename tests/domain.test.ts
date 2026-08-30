import { describe, expect, it } from "vitest";

import { providerLabel } from "../src/services/ipc";

describe("provider labels", () => {
  it("keeps compact badges distinct from provider names", () => {
    expect(providerLabel("local")).toBe("LOCAL");
    expect(providerLabel("youtube")).toBe("YT");
    expect(providerLabel("soundcloud")).toBe("SC");
    expect(providerLabel("spotify")).toBe("SP");
  });
});
