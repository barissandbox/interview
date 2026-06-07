/** Tauri frontend for Interview controls and transcript rendering. */

import * as InterviewBackend from "./backend.js";
import { getRefs } from "./dom.js";
import * as InterviewRender from "./render.js";
import type { UiModel } from "./render.js";
import type { AppViewState } from "./types";

const refs = getRefs();
const model: UiModel = {
  appState: null,
  autoFollowQuestions: true,
  compactHeightDelta: 0,
  compactMode: false,
  interimText: "",
  lastQuestionCount: 0,
  selectedQuestionIndex: -1,
  streamingAnswer: "",
  streamingQuestionId: "",
  transcriptSelectionLocked: false,
};
let lastBalanceRefreshKey = "";
let chatgptLimitRefreshAttempted = false;
let transcriptPointerDown = false;

document.addEventListener("DOMContentLoaded", async () => {
  bindEvents();
  bindSplitter();
  await bindBackendEvents();
  await refreshState();
});

/** Wires UI controls to Tauri commands. */
function bindEvents(): void {
  refs.deepgramLink.addEventListener("click", () => {
    void safeInvoke("open_deepgram_site");
  });

  refs.developerLink.addEventListener("click", () => {
    void safeInvoke("open_developer_site");
  });

  refs.sourceLink.addEventListener("click", () => {
    void safeInvoke("open_source_site");
  });

  refs.testButton.addEventListener("click", async () => {
    const attemptedKey = refs.apiKeyInput.value;
    await saveSettingsOnly();
    const state = await safeInvoke<AppViewState>("test_deepgram_key", {
      apiKey: refs.apiKeyInput.value,
    });
    if (state) {
      renderStateAndUpdate(state);
      refs.apiKeyInput.value = attemptedKey;
      InterviewRender.updateButtons(refs, model);
    }
  });

  refs.chatgptLoginButton.addEventListener("click", async () => {
    await saveSettingsOnly();
    const state = await safeInvoke<AppViewState>("start_chatgpt_login");
    if (state) {
      renderStateAndUpdate(state);
      void refreshDeepgramBalanceIfNeeded(state);
    }
  });

  refs.chatgptSignOutButton.addEventListener("click", async () => {
    const state = await safeInvoke<AppViewState>("sign_out_chatgpt");
    if (state) renderStateAndUpdate(state);
  });

  refs.modelRefreshButton.addEventListener("click", async () => {
    const state = await safeInvoke<AppViewState>("refresh_chatgpt_models");
    if (state) renderStateAndUpdate(state);
  });

  refs.uploadCvButton.addEventListener("click", () => refs.cvFileInput.click());
  refs.cvFileInput.addEventListener("change", () => {
    void uploadCvProfile();
  });
  refs.removeCvButton.addEventListener("click", async () => {
    const state = await safeInvoke<AppViewState>("remove_cv_profile");
    if (state) renderStateAndUpdate(state);
  });

  refs.speakerToggle.addEventListener("click", async () => {
    refs.speakerToggle.classList.toggle("is-on");
    await saveSettingsAndRender();
  });

  refs.microphoneToggle.addEventListener("click", async () => {
    refs.microphoneToggle.classList.toggle("is-on");
    await saveSettingsAndRender();
  });

  refs.apiKeyInput.addEventListener("input", () => {
    InterviewRender.updateButtons(refs, model);
  });
  document.addEventListener("selectionchange", () => {
    InterviewRender.updateButtons(refs, model);
    if (
      !transcriptPointerDown &&
      !InterviewRender.hasSelectionInElement(refs.transcriptText)
    ) {
      model.transcriptSelectionLocked = false;
    }
    InterviewRender.renderTranscript(refs, model);
  });
  refs.transcriptText.addEventListener("pointerdown", () => {
    transcriptPointerDown = true;
    model.transcriptSelectionLocked = true;
  });
  document.addEventListener("pointerup", () => {
    transcriptPointerDown = false;
    if (!InterviewRender.hasSelectionInElement(refs.transcriptText)) {
      model.transcriptSelectionLocked = false;
    }
    InterviewRender.renderTranscript(refs, model);
  });

  for (const element of [
    refs.speakerSelect,
    refs.microphoneSelect,
    refs.languageSelect,
    refs.modelSelect,
    refs.thinkingSelect,
    refs.fastSelect,
    refs.verbositySelect,
    refs.answerTypeSelect,
  ]) {
    element.addEventListener("change", saveSettingsAndRender);
  }
  refs.targetPositionInput.addEventListener("input", () => {
    void saveSettingsOnly();
  });

  refs.previousButton.addEventListener("click", () => navigateTranscript(-1));
  refs.nextButton.addEventListener("click", () => navigateTranscript(1));

  refs.questionPrevButton.addEventListener("click", () => {
    InterviewRender.selectQuestion(refs, model, -1);
  });
  refs.questionNextButton.addEventListener("click", () => {
    InterviewRender.selectQuestion(refs, model, 1);
  });
  refs.questionAutoButton.addEventListener("click", () => {
    InterviewRender.setAutoFollowQuestions(refs, model, !model.autoFollowQuestions);
  });

  refs.compactButton.addEventListener("click", () => {
    void toggleCompactMode();
  });
  refs.alwaysOnTopButton.addEventListener("click", async () => {
    const enabled = !model.appState?.settings.alwaysOnTop;
    const state = await safeInvoke<AppViewState>("set_always_on_top", { enabled });
    if (state) renderStateAndUpdate(state);
  });

  refs.newButton.addEventListener("click", async () => {
    model.interimText = "";
    model.lastQuestionCount = 0;
    model.selectedQuestionIndex = -1;
    const state = await safeInvoke<AppViewState>("create_transcript");
    if (state) renderStateAndUpdate(state);
  });

  refs.deleteButton.addEventListener("click", async () => {
    model.interimText = "";
    model.lastQuestionCount = 0;
    model.selectedQuestionIndex = -1;
    const state = await safeInvoke<AppViewState>("delete_transcript");
    if (state) renderStateAndUpdate(state);
  });

  refs.answerButton.addEventListener("click", async () => {
    const question = selectedAnswerInput();
    clearTextSelection();
    model.transcriptSelectionLocked = false;
    InterviewRender.updateButtons(refs, model);
    await saveSettingsOnly();
    const state = await safeInvoke<AppViewState>("generate_answer", {
      question,
    });
    if (state) renderStateAndUpdate(state);
  });

  refs.startButton.addEventListener("click", async () => {
    await saveSettingsOnly();
    model.interimText = "";
    const state = await safeInvoke<AppViewState>("start_capture");
    if (state) renderStateAndUpdate(state);
  });

  refs.stopButton.addEventListener("click", async () => {
    model.interimText = "";
    const state = await safeInvoke<AppViewState>("stop_capture");
    if (state) renderStateAndUpdate(state);
  });

  refs.copyButton.addEventListener("click", async () => {
    const text = InterviewRender.getCopyText(refs);
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
      InterviewRender.renderCopyFeedback(refs, model);
      InterviewRender.renderStatus(refs, "Copied.");
    } catch (error) {
      InterviewRender.renderStatus(refs, `Copy failed: ${error}`, true);
    }
  });
}

/** Enables vertical resizing between transcript and answer panes. */
function bindSplitter(): void {
  const bodyElement = refs.answerSplitter.parentElement;
  if (!bodyElement) return;

  refs.answerSplitter.addEventListener("pointerdown", (event) => {
    event.preventDefault();
    refs.answerSplitter.setPointerCapture(event.pointerId);
    bodyElement.classList.add("is-resizing");

    const handleMove = (moveEvent: PointerEvent): void => {
      const rect = bodyElement.getBoundingClientRect();
      const minPaneHeight = 72;
      const topHeight = Math.max(
        minPaneHeight,
        Math.min(
          rect.height - 120 - refs.answerSplitter.offsetHeight,
          moveEvent.clientY - rect.top
        )
      );
      bodyElement.style.setProperty("--ct-top-pane-size", `${topHeight}px`);
    };

    const stopResize = (): void => {
      bodyElement.classList.remove("is-resizing");
      window.removeEventListener("pointermove", handleMove);
      window.removeEventListener("pointerup", stopResize);
      window.removeEventListener("pointercancel", stopResize);
    };

    window.addEventListener("pointermove", handleMove);
    window.addEventListener("pointerup", stopResize);
    window.addEventListener("pointercancel", stopResize);
  });
}

/** Registers backend event listeners. */
async function bindBackendEvents(): Promise<void> {
  await InterviewBackend.listenTranscriptEvents((payload) => {
    if (payload.type === "status") {
      InterviewRender.renderStatus(refs, payload.message);
    } else if (payload.type === "interim") {
      model.interimText = payload.text || "";
      InterviewRender.renderTranscript(refs, model);
    } else if (payload.type === "answer") {
      InterviewRender.renderStreamingAnswer(
        refs,
        model,
        payload.questionId,
        payload.question,
        payload.answer
      );
    } else if (payload.type === "state") {
      model.interimText = "";
      renderStateAndUpdate(payload.state);
      void refreshDeepgramBalanceIfNeeded(payload.state);
    } else if (payload.type === "error") {
      InterviewRender.renderStatus(refs, payload.message, true);
    }
  });
}

/** Safely invokes a Tauri command, catching and displaying errors. */
async function safeInvoke<T = void>(
  command: string,
  args?: Record<string, unknown>
): Promise<T | null> {
  try {
    return await InterviewBackend.invokeCommand<T>(command, args);
  } catch (error) {
    InterviewRender.renderStatus(refs, String(error), true);
    InterviewRender.updateButtons(refs, model);
    return null;
  }
}

/** Fetches current state from Rust. */
async function refreshState(): Promise<void> {
  const state = await safeInvoke<AppViewState>("get_app_state");
  if (state) {
    renderStateAndUpdate(state);
    void refreshDeepgramBalanceIfNeeded(state);
    void refreshChatgptLimitsIfStale(state);
  }
}

/** Refreshes a saved Deepgram balance once per visible API key. */
async function refreshDeepgramBalanceIfNeeded(state: AppViewState): Promise<void> {
  const apiKey = state.settings.apiKey.trim();
  if (!apiKey || state.balance || lastBalanceRefreshKey === apiKey) return;
  lastBalanceRefreshKey = apiKey;
  const refreshed = await safeInvoke<AppViewState>("test_deepgram_key", { apiKey });
  if (refreshed) {
    renderStateAndUpdate({
      ...refreshed,
      status: state.status || "Ready.",
    });
  }
}

/** Refreshes ChatGPT limits when persisted catalog data looks stale. */
async function refreshChatgptLimitsIfStale(state: AppViewState): Promise<void> {
  if (
    chatgptLimitRefreshAttempted ||
    !state.chatgpt.loggedIn ||
    /\bresets\b/i.test(state.chatgpt.limitLabel)
  ) {
    return;
  }
  chatgptLimitRefreshAttempted = true;
  try {
    const refreshed = await InterviewBackend.invokeCommand<AppViewState>("refresh_chatgpt_models");
    renderStateAndUpdate(refreshed);
  } catch {
    // Keep startup quiet when the usage endpoint is temporarily unavailable.
  }
}

/** Toggles compact layout and resizes the native window to match. */
async function toggleCompactMode(): Promise<void> {
  const enabling = !model.compactMode;
  if (enabling) {
    const beforeControls = refs.controlsPanel.getBoundingClientRect().height;
    const beforeLimit = refs.limitRow.getBoundingClientRect().height;
    const beforeFooter = refs.appFooter.getBoundingClientRect().height;
    InterviewRender.setCompactMode(refs, model, true);
    await nextAnimationFrame();
    const afterControls = refs.controlsPanel.getBoundingClientRect().height;
    const delta = Math.max(0, Math.round(beforeLimit + beforeFooter + beforeControls - afterControls));
    model.compactHeightDelta = delta;
    if (delta > 0) {
      await safeInvoke("resize_window_height", { delta: -delta });
    }
    return;
  }

  const delta = Math.max(0, Math.round(model.compactHeightDelta));
  InterviewRender.setCompactMode(refs, model, false);
  await nextAnimationFrame();
  if (delta > 0) {
    await safeInvoke("resize_window_height", { delta });
  }
  model.compactHeightDelta = 0;
}

/** Waits for one browser paint tick. */
function nextAnimationFrame(): Promise<void> {
  return new Promise((resolve) => window.requestAnimationFrame(() => resolve()));
}

/** Navigates to a transcript by offset. */
async function navigateTranscript(offset: number): Promise<void> {
  model.interimText = "";
  model.lastQuestionCount = 0;
  model.selectedQuestionIndex = -1;
  const state = await safeInvoke<AppViewState>("select_transcript_by_offset", {
    offset,
  });
  if (state) renderStateAndUpdate(state);
}

/** Saves settings and renders returned state. */
async function saveSettingsAndRender(): Promise<void> {
  const state = await safeInvoke<AppViewState>("save_settings", {
    settings: InterviewRender.collectSettings(refs),
  });
  if (state) {
    renderStateAndUpdate(state);
  }
}

/** Saves settings without rerendering the whole tree. */
async function saveSettingsOnly(): Promise<void> {
  await safeInvoke("save_settings", {
    settings: InterviewRender.collectSettings(refs),
  });
}

/** Uploads a selected PDF CV to the backend. */
async function uploadCvProfile(): Promise<void> {
  const file = refs.cvFileInput.files?.[0];
  refs.cvFileInput.value = "";
  if (!file) return;
  const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
  const state = await safeInvoke<AppViewState>("upload_cv_profile", {
    fileName: file.name,
    bytes,
  });
  if (state) renderStateAndUpdate(state);
}

/** Renders state and synchronizes derived UI effects. */
function renderStateAndUpdate(state: AppViewState): void {
  InterviewRender.renderState(refs, model, state);
}

/** Returns selected text from the page, constrained for answer generation. */
function selectedAnswerInput(): string {
  return window.getSelection()?.toString().trim().slice(0, 2000) || "";
}

/** Clears the current browser text selection. */
function clearTextSelection(): void {
  window.getSelection()?.removeAllRanges();
}
