/* global console, process, window */

import { chromium, expect } from "@playwright/test";

const cdpUrl = process.env.SPOTDIY_PACKAGED_CDP_URL;
const fixtureFolder = process.env.SPOTDIY_PACKAGED_FIXTURE;

if (!cdpUrl || !fixtureFolder) {
  throw new Error("SPOTDIY_PACKAGED_CDP_URL and SPOTDIY_PACKAGED_FIXTURE are required");
}

const browser = await chromium.connectOverCDP(cdpUrl);
try {
  const pages = browser.contexts().flatMap((context) => context.pages());
  const page = pages[0];
  if (!page) {
    throw new Error("the packaged app did not expose a WebView page");
  }

  async function invoke(command, args = undefined) {
    return page.evaluate(async ({ command: commandName, args: commandArgs }) => {
      const internals = window.__TAURI_INTERNALS__;
      if (!internals || typeof internals.invoke !== "function") {
        throw new Error("the packaged page does not expose the Tauri invoke bridge");
      }
      return internals.invoke(commandName, commandArgs);
    }, { command, args });
  }

  async function waitFor(description, predicate, timeoutMs = 20_000) {
    const deadline = Date.now() + timeoutMs;
    let lastValue;
    while (Date.now() < deadline) {
      lastValue = await predicate();
      if (lastValue) {
        return lastValue;
      }
      await page.waitForTimeout(250);
    }
    throw new Error(`timed out waiting for ${description}; last value: ${JSON.stringify(lastValue)}`);
  }

  await page.waitForLoadState("domcontentloaded");
  await page.getByRole("link", { name: "Your library", exact: true }).waitFor({ state: "visible", timeout: 20_000 });

  await invoke("add_library_folders", { paths: [fixtureFolder] });
  await waitFor("the isolated synthetic library", async () => {
    const status = await invoke("get_library_status");
    return status.folders?.length === 1 && status.indexedTrackCount >= 2 && !status.isScanning ? status : false;
  });

  const appStatus = await invoke("get_app_status");
  const providers = new Map(appStatus.providers.map((provider) => [provider.kind, provider]));
  if (providers.get("spotify")?.runtimeStatus !== "disabled") {
    throw new Error("Spotify was not disabled in the packaged smoke profile");
  }
  if (providers.get("youtube")?.runtimeStatus !== "missing" || providers.get("soundcloud")?.runtimeStatus !== "missing") {
    throw new Error("yt-dlp-backed providers were not reported missing in the isolated profile");
  }

  await page.getByRole("link", { name: "Search", exact: true }).click();
  await page.getByRole("heading", { name: /Find your next listen/ }).waitFor({ state: "visible" });
  const input = page.getByRole("textbox", { name: "Search music" });
  await input.fill("night");

  const local = page.locator('[data-provider="local"]');
  await expect(local.locator(".search-result-card").first()).toBeVisible({ timeout: 20_000 });
  await expect(local).toContainText("Night Drive");
  await expect(page.locator('[data-provider="youtube"]')).toContainText(/yt-dlp|unavailable|missing/i);
  await expect(page.locator('[data-provider="soundcloud"]')).toContainText(/yt-dlp|unavailable|missing/i);

  const startedPromise = invoke("start_search", {
    request: {
      query: "cancellation-check",
      lens: "all",
      sortField: "relevance",
      sortDirection: "descending",
      limit: 25,
    },
  });
  const cancelledPromise = invoke("cancel_search");
  const [started, cancelled] = await Promise.all([startedPromise, cancelledPromise]);
  if (!started.searchId) {
    throw new Error("packaged search did not return a SearchId");
  }
  if (cancelled !== started.searchId) {
    throw new Error("packaged cancellation did not return the active SearchId");
  }

  console.log("packaged provider search flow passed");
} finally {
  await browser.close();
}
