/* global console, process, window */

import { chromium } from "@playwright/test";

const phase = process.argv[2];
const cdpUrl = process.env.SPOTDIY_PACKAGED_CDP_URL;

if (!cdpUrl) {
  throw new Error("SPOTDIY_PACKAGED_CDP_URL is required");
}
if (!["standard", "portable", "standard-final"].includes(phase)) {
  throw new Error(`unknown Plan 13 packaged smoke phase: ${phase}`);
}

const browser = await chromium.connectOverCDP(cdpUrl);
try {
  const page = browser.contexts().flatMap((context) => context.pages())[0];
  if (!page) {
    throw new Error("the packaged app did not expose a WebView page");
  }

  async function invoke(command, args = undefined) {
    const result = await page.evaluate(async ({ command: commandName, args: commandArgs }) => {
      try {
        const internals = window.__TAURI_INTERNALS__;
        if (!internals || typeof internals.invoke !== "function") {
          throw new Error("the packaged page does not expose the Tauri invoke bridge");
        }
        return { ok: true, value: await internals.invoke(commandName, commandArgs) };
      } catch (error) {
        return { ok: false, error: String(error) };
      }
    }, { command, args });
    if (!result.ok) {
      throw new Error(`${command} failed: ${result.error}`);
    }
    return result.value;
  }

  await page.waitForLoadState("domcontentloaded");
  await page.getByRole("link", { name: "Your library", exact: true }).waitFor({ state: "visible", timeout: 20_000 });

  const status = await invoke("get_storage_status");
  if (phase === "standard") {
    if (status.mode !== "standard" || status.portableMarkerPresent || status.restartRequired) {
      throw new Error(`standard startup status was not deterministic: ${JSON.stringify(status)}`);
    }
    const switched = await invoke("prepare_storage_mode_switch", { targetMode: "portable" });
    if (switched.mode !== "portable" || switched.restartRequired !== true) {
      throw new Error(`standard-to-portable preparation was not restart-gated: ${JSON.stringify(switched)}`);
    }
    console.log("packaged Plan 13 Standard-to-Portable preparation passed");
  } else if (phase === "portable") {
    if (status.mode !== "portable" || !status.portableMarkerPresent || status.restartRequired) {
      throw new Error(`portable startup status was not deterministic: ${JSON.stringify(status)}`);
    }
    if (!status.databasePath.replaceAll("/", "\\").toLowerCase().endsWith("\\database\\spotdiy.sqlite3")) {
      throw new Error(`portable startup did not select the executable Database path: ${JSON.stringify(status)}`);
    }
    const switched = await invoke("prepare_storage_mode_switch", { targetMode: "standard" });
    if (switched.mode !== "standard" || switched.restartRequired !== true) {
      throw new Error(`portable-to-standard preparation was not restart-gated: ${JSON.stringify(switched)}`);
    }
    console.log("packaged Plan 13 Portable-to-Standard preparation passed");
  } else if (status.mode !== "standard" || status.portableMarkerPresent || status.restartRequired) {
    throw new Error(`final standard startup status was not deterministic: ${JSON.stringify(status)}`);
  } else {
    console.log("packaged Plan 13 final Standard restart passed");
  }
} finally {
  await browser.close();
}
