/// Bridge selection: real Tauri commands inside the webview, canvas mocks
/// in a plain browser tab. Detection is the standard `__TAURI_INTERNALS__`
/// probe; nothing Tauri-specific runs until a command is invoked.
import { BrowserBridge } from "$lib/bridge/mock";
import { tauriBridge } from "$lib/bridge/tauri";
import type { Bridge } from "$lib/bridge/index";

/** Whether the frontend is running inside the Tauri webview. */
export function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** The live bridge for this environment. */
export function getBridge(): Bridge {
  return inTauri() ? tauriBridge : new BrowserBridge();
}
