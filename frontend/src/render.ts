/** Rendering and UI state helpers for Interview. */

import type { DomRefs } from "./dom.js";
import type {
  AppViewState,
  AudioDevice,
  AvailableModel,
  FrontendSettings,
  SelectOption,
} from "./types";

export interface UiModel {
  appState: AppViewState | null;
  autoFollowQuestions: boolean;
  compactHeightDelta: number;
  compactMode: boolean;
  interimText: string;
  lastQuestionCount: number;
  selectedQuestionIndex: number;
  streamingAnswer: string;
  streamingQuestionId: string;
  transcriptSelectionLocked: boolean;
  copyResetTimer?: number;
}

/** Collects current UI settings into the FrontendSettings shape. */
export function collectSettings(refs: DomRefs): FrontendSettings {
  return {
    speakerDeviceId: refs.speakerSelect.value,
    microphoneDeviceId: refs.microphoneSelect.value,
    language: refs.languageSelect.value,
    speakerEnabled: refs.speakerToggle.classList.contains("is-on"),
    microphoneEnabled: refs.microphoneToggle.classList.contains("is-on"),
    model: refs.modelSelect.value,
    thinkingVariant: refs.thinkingSelect.value,
    answerType: refs.answerTypeSelect.value,
    fastEnabled: refs.fastSelect.value !== "normal",
    verbosity: refs.verbositySelect.value,
    targetPosition: refs.targetPositionInput.value,
    alwaysOnTop: Boolean(modelStateAlwaysOnTop(refs)),
  };
}

/** Renders all UI fragments from state. */
export function renderState(
  refs: DomRefs,
  model: UiModel,
  state: AppViewState
): void {
  const previousQuestionCount = model.lastQuestionCount;
  const previousState = model.appState;
  model.appState = state;
  const pendingIndex = latestPendingQuestionIndex(state, previousState);
  if (pendingIndex >= 0 && (model.autoFollowQuestions || model.selectedQuestionIndex < 0)) {
    model.selectedQuestionIndex = pendingIndex;
  } else if (model.autoFollowQuestions && state.questions.length > previousQuestionCount) {
    model.selectedQuestionIndex = state.questions.length - 1;
  } else {
    model.selectedQuestionIndex = normalizeQuestionIndex(
      model.selectedQuestionIndex,
      state.questions.length,
      state.selectedQuestionIndex
    );
  }
  model.lastQuestionCount = state.questions.length;
  const selected = state.questions[model.selectedQuestionIndex];
  model.streamingQuestionId = "";
  model.streamingAnswer = "";
  populateOptions(refs, model);
  renderStatus(refs, state.status);
  renderLimits(refs, state);
  renderAccount(refs, state);
  renderProfile(refs, state);
  renderToggles(refs, model);
  renderTranscriptHeader(refs, model);
  renderTranscript(refs, model);
  renderAnswer(refs, model);
  if (selected) {
    refs.questionText.textContent = selected.text;
  }
  updateButtons(refs, model);
}

/** Renders current status text. */
export function renderStatus(
  refs: DomRefs,
  message: string,
  isError = false
): void {
  refs.statusText.textContent = message || "Ready.";
  refs.statusRow.classList.toggle("is-error", isError);
}

/** Renders account and balance data in the dedicated limit row. */
export function renderLimits(refs: DomRefs, state: AppViewState): void {
  const account = state.chatgpt.loggedIn ? state.chatgpt.accountEmail.trim() : "";
  refs.chatgptLimitText.textContent = [account, state.chatgpt.limitLabel]
    .filter(Boolean)
    .join(" ") || "--";
  refs.deepgramLimitText.textContent = formatDeepgramLimit(state.balance, state.settings.apiKey);
}

/** Applies a streaming answer event without waiting for full state refresh. */
export function renderStreamingAnswer(
  refs: DomRefs,
  model: UiModel,
  questionId: string,
  question: string,
  answer: string
): void {
  model.streamingQuestionId = questionId;
  model.streamingAnswer = answer;
  const index = model.appState?.questions.findIndex((item) => item.id === questionId) ?? -1;
  if (index >= 0 && (model.autoFollowQuestions || model.selectedQuestionIndex < 0)) {
    model.selectedQuestionIndex = index;
  }
  if (index >= 0 && model.selectedQuestionIndex === index) {
    renderAnswer(refs, model);
  } else if (!model.appState || (index < 0 && model.autoFollowQuestions)) {
    refs.questionRow.hidden = false;
    refs.questionText.textContent = question;
    renderAnswerMarkdown(refs.answerText, answer || "", !answer);
    refs.answerText.scrollTop = refs.answerText.scrollHeight;
  }
  renderStatus(refs, "Streaming answer...");
  updateButtons(refs, model);
}

/** Shows a temporary copied state on the copy button. */
export function renderCopyFeedback(
  refs: DomRefs,
  model: UiModel
): void {
  window.clearTimeout(model.copyResetTimer);
  refs.copyButton.classList.add("is-copied");
  refs.copyButton.textContent = "\u2713";
  model.copyResetTimer = window.setTimeout(() => {
    refs.copyButton.classList.remove("is-copied");
    refs.copyButton.textContent = "\u29c9";
    updateButtons(refs, model);
  }, 1000);
}

/** Builds clipboard text with transcript, question, and answer. */
export function getCopyText(refs: DomRefs): string {
  const transcript = refs.transcriptText.classList.contains("ct-empty")
    ? ""
    : refs.transcriptText.textContent?.trim() || "";
  const question = refs.questionText.textContent?.trim() || "";
  const answer = refs.answerText.classList.contains("ct-empty")
    ? ""
    : refs.answerText.textContent?.trim() || "";
  return [transcript, question ? `Question: ${question}` : "", answer ? `Answer:\n${answer}` : ""]
    .filter(Boolean)
    .join("\n\n");
}

/** Renders saved and interim transcript text. */
export function renderTranscript(
  refs: DomRefs,
  model: UiModel
): void {
  if (model.transcriptSelectionLocked || hasSelectionInElement(refs.transcriptText)) {
    return;
  }
  const parts: string[] = [];
  if (model.appState?.transcriptText) {
    parts.push(model.appState.transcriptText.trim());
  }
  if (model.interimText) {
    parts.push(model.interimText.trim());
  }
  const text = parts.filter(Boolean).join(" ");
  refs.transcriptText.textContent = text || "No transcript yet.";
  refs.transcriptText.classList.toggle("ct-empty", !text);
  refs.transcriptText.scrollTop = refs.transcriptText.scrollHeight;
}

/** Renders selected question and answer text. */
export function renderAnswer(
  refs: DomRefs,
  model: UiModel
): void {
  const state = model.appState;
  const selected = state?.questions[model.selectedQuestionIndex];
  refs.questionRow.hidden = !selected;
  if (!selected) {
    refs.questionCounter.textContent = "0/0";
    refs.questionText.textContent = "";
    renderAnswerMarkdown(refs.answerText, "No answer yet.", true);
    return;
  }
  const isStreaming = model.streamingQuestionId === selected.id && model.streamingAnswer;
  const answer = isStreaming ? model.streamingAnswer : selected.answer;
  refs.questionCounter.textContent = `${model.selectedQuestionIndex + 1}/${state?.questions.length ?? 0}`;
  refs.questionText.textContent = selected.text;
  renderAnswerMarkdown(
    refs.answerText,
    answer || (selected.pending ? "" : "No answer yet."),
    !answer
  );
  refs.answerText.scrollTop = refs.answerText.scrollHeight;
}

/** Moves the selected question by offset. */
export function selectQuestion(
  refs: DomRefs,
  model: UiModel,
  offset: number
): void {
  const count = model.appState?.questions.length ?? 0;
  if (count === 0) {
    model.selectedQuestionIndex = -1;
  } else {
    model.selectedQuestionIndex = Math.max(0, Math.min(count - 1, model.selectedQuestionIndex + offset));
  }
  renderAnswer(refs, model);
  updateButtons(refs, model);
}

/** Updates button visibility and disabled states. */
export function updateButtons(
  refs: DomRefs,
  model: UiModel
): void {
  if (!model.appState) return;
  const running = Boolean(model.appState.running);
  const apiKeyReady = isApiKeyReady(refs, model);
  const chatgptReady = Boolean(model.appState.chatgpt.loggedIn);
  const hasSelectedText = Boolean(selectedAnswerText());
  const sourceReady =
    refs.speakerToggle.classList.contains("is-on") ||
    refs.microphoneToggle.classList.contains("is-on");
  refs.startButton.hidden = running;
  refs.stopButton.hidden = !running;
  refs.startButton.disabled = running || !apiKeyReady || !sourceReady || !chatgptReady;
  refs.stopButton.disabled = !running;
  refs.previousButton.disabled = running || model.appState.activeIndex <= 0;
  refs.nextButton.disabled =
    running || model.appState.activeIndex >= model.appState.transcriptCount - 1;
  refs.newButton.disabled = running;
  refs.deleteButton.disabled = running || isLastEmptyTranscript(model);
  refs.languageSelect.disabled = running;
  refs.answerTypeSelect.disabled = running;
  refs.targetPositionInput.disabled = running;
  refs.speakerSelect.disabled = running;
  refs.microphoneSelect.disabled = running;
  refs.apiKeyInput.disabled = running;
  refs.testButton.disabled = running || !refs.apiKeyInput.value.trim();
  refs.chatgptLoginButton.hidden = chatgptReady;
  refs.chatgptSignOutButton.hidden = !chatgptReady;
  refs.chatgptSignOutButton.disabled = running || !chatgptReady;
  refs.modelSelect.hidden = !chatgptReady;
  refs.modelSelect.disabled = running || !chatgptReady;
  refs.thinkingSelect.hidden = !chatgptReady;
  refs.thinkingSelect.disabled = running || !chatgptReady;
  refs.fastSelect.hidden = !chatgptReady;
  refs.fastSelect.disabled = running || !chatgptReady;
  refs.verbositySelect.hidden = !chatgptReady;
  refs.verbositySelect.disabled = running || !chatgptReady;
  refs.modelRefreshButton.hidden = !chatgptReady;
  refs.modelRefreshButton.disabled = running || !chatgptReady;
  refs.uploadCvButton.disabled = running;
  refs.alwaysOnTopButton.disabled = running;
  refs.answerButton.disabled =
    !chatgptReady || Boolean(model.appState.answerPending) || !hasSelectedText;
  refs.answerButton.title = hasSelectedText ? "Generate answer" : "Select text first.";
  refs.compactButton.textContent = model.compactMode ? "Full" : "Compact";
  refs.alwaysOnTopButton.classList.toggle(
    "is-active",
    Boolean(model.appState.settings.alwaysOnTop)
  );
  refs.alwaysOnTopButton.setAttribute(
    "aria-pressed",
    String(Boolean(model.appState.settings.alwaysOnTop))
  );
  refs.questionAutoButton.textContent = model.autoFollowQuestions ? "||" : "\u25b6";
  refs.questionAutoButton.classList.toggle("is-active", model.autoFollowQuestions);
  refs.questionPrevButton.disabled = model.selectedQuestionIndex <= 0;
  refs.questionNextButton.disabled =
    model.selectedQuestionIndex < 0 ||
    model.selectedQuestionIndex >= model.appState.questions.length - 1;
  refs.removeCvButton.hidden = !model.appState.profile.textLength;
  refs.removeCvButton.disabled = running || !model.appState.profile.textLength;
  refs.copyButton.disabled = !getCopyText(refs);
}

/** Populates device, language, model, and thinking selects. */
function populateOptions(
  refs: DomRefs,
  model: UiModel
): void {
  if (!model.appState) return;

  replaceOptions(
    refs.languageSelect,
    model.appState.languages.map((item) => ({
      value: item.value,
      label: item.label,
    }))
  );
  replaceOptions(
    refs.modelSelect,
    model.appState.models
      .slice()
      .filter((item) => !item.hidden && item.inputModalities.includes("text"))
      .sort(compareModels)
      .map((item) => ({
        value: item.model,
        label: item.model,
        title: item.description || item.model,
      }))
  );
  replaceOptions(
    refs.thinkingSelect,
    model.appState.thinkingVariants.map((item) => ({
      value: item.value,
      label: item.value,
      title: item.description,
    }))
  );
  replaceOptions(
    refs.speakerSelect,
    model.appState.devices
      .filter((item) => item.kind === "Speaker")
      .map(deviceOption)
  );
  replaceOptions(
    refs.microphoneSelect,
    model.appState.devices
      .filter((item) => item.kind === "Microphone")
      .map(deviceOption)
  );

  refs.apiKeyInput.value = model.appState.settings.apiKey || "";
  selectPreferredDevice(
    refs.speakerSelect,
    model.appState.settings.speakerDeviceId || ""
  );
  selectPreferredDevice(
    refs.microphoneSelect,
    model.appState.settings.microphoneDeviceId || ""
  );
  refs.languageSelect.value = model.appState.settings.language || "en-US";
  refs.modelSelect.value = model.appState.settings.model || "gpt-5.4-mini";
  refs.thinkingSelect.value = model.appState.settings.thinkingVariant || "low";
  refs.answerTypeSelect.value = model.appState.settings.answerType || "details";
  refs.fastSelect.value = model.appState.settings.fastEnabled === false ? "normal" : "fast";
  refs.verbositySelect.value = model.appState.settings.verbosity || "low";
  refs.targetPositionInput.value = model.appState.settings.targetPosition || "";
}

/** Keeps the legacy account label hidden while limits show account data. */
function renderAccount(refs: DomRefs, _state: AppViewState): void {
  refs.accountLabel.hidden = true;
  refs.accountLabel.textContent = "";
}

/** Applies compact layout state to the shell. */
export function setCompactMode(
  refs: DomRefs,
  model: UiModel,
  enabled: boolean
): void {
  model.compactMode = enabled;
  refs.appShell.classList.toggle("is-compact", enabled);
  updateButtons(refs, model);
}

/** Toggles automatic selection of the latest question. */
export function setAutoFollowQuestions(
  refs: DomRefs,
  model: UiModel,
  enabled: boolean
): void {
  model.autoFollowQuestions = enabled;
  if (enabled && model.appState?.questions.length) {
    model.selectedQuestionIndex = model.appState.questions.length - 1;
    renderAnswer(refs, model);
  }
  updateButtons(refs, model);
}

/** Renders local CV/profile status text. */
function renderProfile(refs: DomRefs, state: AppViewState): void {
  if (!state.profile.textLength) {
    refs.cvStatus.textContent = "No CV loaded.";
    return;
  }
  refs.cvStatus.textContent = `${state.profile.fileName || "CV"} loaded locally (${state.profile.textLength} chars).`;
}

/** Selects a saved device or falls back to the first option. */
function selectPreferredDevice(
  select: HTMLSelectElement,
  savedValue: string
): void {
  if (
    savedValue &&
    Array.from(select.options).some((option) => option.value === savedValue)
  ) {
    select.value = savedValue;
    return;
  }
  if (select.options.length > 0) {
    select.selectedIndex = 0;
  }
}

/** Replaces select options without preserving stale values. */
function replaceOptions(
  select: HTMLSelectElement,
  options: SelectOption[]
): void {
  const previous = select.value;
  select.innerHTML = "";
  for (const option of options) {
    const element = document.createElement("option");
    element.value = option.value;
    element.textContent = option.label;
    element.title = option.title || option.label;
    select.appendChild(element);
  }
  if (options.some((option) => option.value === previous)) {
    select.value = previous;
  }
}

/** Creates a select option from an audio device. */
function deviceOption(device: AudioDevice): SelectOption {
  const suffix = device.isDefault ? " (Default)" : "";
  const label = cleanDeviceName(device.name) || device.name || device.id;
  return {
    value: device.id,
    label: `${label}${suffix}`,
    title: `${device.name}${suffix}`,
  };
}

/** Removes noisy Windows endpoint suffixes from device names. */
function cleanDeviceName(name: string): string {
  return String(name || "")
    .replace(
      /\s*\((?:Realtek\(R\)|NVIDIA|AMD|Intel\(R\)|High Definition Audio Device|USB Audio Device)[^)]+\)/gi,
      ""
    )
    .replace(/\s{2,}/g, " ")
    .trim();
}

/** Renders toggle state. */
function renderToggles(refs: DomRefs, model: UiModel): void {
  if (!model.appState) return;
  setToggle(refs.speakerToggle, Boolean(model.appState.settings.speakerEnabled));
  setToggle(
    refs.microphoneToggle,
    Boolean(model.appState.settings.microphoneEnabled)
  );
}

/** Sets one switch visual state. */
function setToggle(element: HTMLElement, enabled: boolean): void {
  element.classList.toggle("is-on", enabled);
  element.setAttribute("aria-pressed", String(enabled));
}

/** Renders transcript header metadata. */
function renderTranscriptHeader(
  refs: DomRefs,
  model: UiModel
): void {
  if (!model.appState) return;
  const count = model.appState.transcriptCount || 0;
  const index = count ? model.appState.activeIndex + 1 : 0;
  refs.transcriptCounter.textContent = `${index}/${count}`;
  const active = model.appState.transcripts[model.appState.activeIndex];
  refs.transcriptMeta.textContent = active
    ? active.label
    : "No transcript selected";
}

/** Returns true when the visible key matches the last tested and saved key. */
function isApiKeyReady(
  refs: DomRefs,
  model: UiModel
): boolean {
  const saved = model.appState?.settings?.apiKey || "";
  return Boolean(saved.trim()) && refs.apiKeyInput.value.trim() === saved.trim();
}

/** Returns true when the only transcript has no saved text or questions. */
function isLastEmptyTranscript(model: UiModel): boolean {
  const state = model.appState;
  if (!state) return false;
  return (
    state.transcriptCount === 1 &&
    !state.transcriptText.trim() &&
    state.questions.length === 0
  );
}

/** Keeps selected question index inside available bounds. */
function normalizeQuestionIndex(current: number, count: number, fallback: number): number {
  if (count <= 0) return -1;
  if (current >= 0 && current < count) return current;
  if (fallback >= 0 && fallback < count) return fallback;
  return count - 1;
}

/** Formats the Deepgram balance label for the limit row. */
function formatDeepgramLimit(balance: string, apiKey: string): string {
  if (!balance) {
    return apiKey ? "Deepgram: --" : "Deepgram: --";
  }
  return balance.replace(/^Deepgram:\s*\$(\d+(?:\.\d+)?)/i, "Deepgram: $1$");
}

/** Returns the current selected page text for answer gating. */
function selectedAnswerText(): string {
  return window.getSelection()?.toString().trim() || "";
}

/** Reads the always-on-top visual state from the toggle. */
function modelStateAlwaysOnTop(refs: DomRefs): boolean {
  return refs.alwaysOnTopButton.classList.contains("is-active");
}

/** Returns true when the active text selection intersects an element. */
export function hasSelectionInElement(element: HTMLElement): boolean {
  const selection = window.getSelection();
  if (!selection || selection.isCollapsed || !selection.toString().trim()) {
    return false;
  }
  for (let index = 0; index < selection.rangeCount; index += 1) {
    if (selection.getRangeAt(index).intersectsNode(element)) {
      return true;
    }
  }
  return false;
}

/** Finds the newest question that newly entered pending state. */
function latestPendingQuestionIndex(state: AppViewState, previousState: AppViewState | null): number {
  for (let index = state.questions.length - 1; index >= 0; index -= 1) {
    const question = state.questions[index];
    if (!question?.pending) continue;
    const previous = previousState?.questions.find((item) => item.id === question.id);
    if (!previous || !previous.pending) {
      return index;
    }
  }
  return -1;
}

/** Renders the small supported answer markdown subset. */
function renderAnswerMarkdown(container: HTMLElement, markdown: string, empty: boolean): void {
  container.classList.toggle("ct-empty", empty);
  container.innerHTML = "";
  const text = markdown.trim();
  if (!text) return;
  if (empty) {
    container.textContent = text;
    return;
  }
  const blocks = text.split(/\n{2,}/);
  for (const block of blocks) {
    const lines = block.split("\n");
    if (lines.every((line) => /^\s*[-*]\s+/.test(line))) {
      const list = document.createElement("ul");
      for (const line of lines) {
        const item = document.createElement("li");
        appendInlineMarkdown(item, line.replace(/^\s*[-*]\s+/, ""));
        list.appendChild(item);
      }
      container.appendChild(list);
      continue;
    }
    if (lines.every((line) => /^\s*\d+[.)]\s+/.test(line))) {
      const list = document.createElement("ol");
      for (const line of lines) {
        const item = document.createElement("li");
        appendInlineMarkdown(item, line.replace(/^\s*\d+[.)]\s+/, ""));
        list.appendChild(item);
      }
      container.appendChild(list);
      continue;
    }
    const heading = block.match(/^\s{0,3}(#{1,3})\s+(.+)$/);
    if (heading) {
      const element = document.createElement(heading[1]?.length === 1 ? "h3" : "h4");
      appendInlineMarkdown(element, heading[2] || "");
      container.appendChild(element);
      continue;
    }
    const paragraph = document.createElement("p");
    appendInlineMarkdown(paragraph, block.replace(/\n/g, "\n"));
    container.appendChild(paragraph);
  }
}

/** Appends inline code and emphasis nodes for supported markdown spans. */
function appendInlineMarkdown(parent: HTMLElement, text: string): void {
  const pattern = /(`[^`]+`|\*\*[^*]+\*\*|\*[^*]+\*)/g;
  let cursor = 0;
  for (const match of text.matchAll(pattern)) {
    const index = match.index || 0;
    if (index > cursor) {
      parent.appendChild(document.createTextNode(text.slice(cursor, index)));
    }
    const token = match[0];
    const element = document.createElement(token.startsWith("`") ? "code" : "strong");
    element.textContent = token.startsWith("`")
      ? token.slice(1, -1)
      : token.replace(/^\*\*?|\*\*?$/g, "");
    parent.appendChild(element);
    cursor = index + token.length;
  }
  if (cursor < text.length) {
    parent.appendChild(document.createTextNode(text.slice(cursor)));
  }
}

/** Sorts newer and non-mini models first for the model select. */
function compareModels(a: AvailableModel, b: AvailableModel): number {
  const left = modelSortParts(a.model);
  const right = modelSortParts(b.model);
  for (let index = 0; index < Math.max(left.numbers.length, right.numbers.length); index += 1) {
    const diff = (right.numbers[index] || 0) - (left.numbers[index] || 0);
    if (diff !== 0) return diff;
  }
  if (left.mini !== right.mini) return left.mini ? 1 : -1;
  return a.model.localeCompare(b.model);
}

/** Extracts model version numbers and mini marker for sorting. */
function modelSortParts(value: string): { numbers: number[]; mini: boolean } {
  const numbers = (String(value).match(/\d+(?:\.\d+)?/g) || []).map(Number);
  return {
    numbers,
    mini: /\bmini\b/i.test(value),
  };
}
