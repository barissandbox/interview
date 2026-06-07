/** Tauri backend access for the Interview UI. */

import type { UiEventPayload } from "./types";

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

/** Invokes one Rust command through Tauri. */
export function invokeCommand<T = void>(
  command: string,
  args?: Record<string, unknown>
): Promise<T> {
  return invoke<T>(command, args);
}

/** Subscribes to backend transcript and answer events. */
export function listenTranscriptEvents(
  handler: (payload: UiEventPayload) => void
): Promise<() => void> {
  return listen<UiEventPayload>("transcript-event", (event) => {
    handler(event.payload);
  });
}
