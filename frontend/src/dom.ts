/** Typed DOM lookup for the Interview UI. */

export interface DomRefs {
  appShell: HTMLElement;
  appFooter: HTMLElement;
  statusRow: HTMLElement;
  statusText: HTMLElement;
  limitRow: HTMLElement;
  chatgptLimitText: HTMLElement;
  deepgramLimitText: HTMLElement;
  accountLabel: HTMLElement;
  controlsPanel: HTMLElement;
  deepgramLink: HTMLElement;
  developerLink: HTMLButtonElement;
  sourceLink: HTMLButtonElement;
  apiKeyInput: HTMLInputElement;
  testButton: HTMLButtonElement;
  chatgptLoginButton: HTMLButtonElement;
  chatgptSignOutButton: HTMLButtonElement;
  modelRefreshButton: HTMLButtonElement;
  cvFileInput: HTMLInputElement;
  uploadCvButton: HTMLButtonElement;
  removeCvButton: HTMLButtonElement;
  cvStatus: HTMLElement;
  speakerToggle: HTMLElement;
  microphoneToggle: HTMLElement;
  speakerSelect: HTMLSelectElement;
  microphoneSelect: HTMLSelectElement;
  languageSelect: HTMLSelectElement;
  modelSelect: HTMLSelectElement;
  thinkingSelect: HTMLSelectElement;
  fastSelect: HTMLSelectElement;
  verbositySelect: HTMLSelectElement;
  targetPositionInput: HTMLInputElement;
  answerTypeSelect: HTMLSelectElement;
  previousButton: HTMLButtonElement;
  nextButton: HTMLButtonElement;
  newButton: HTMLButtonElement;
  deleteButton: HTMLButtonElement;
  answerButton: HTMLButtonElement;
  compactButton: HTMLButtonElement;
  alwaysOnTopButton: HTMLButtonElement;
  startButton: HTMLButtonElement;
  stopButton: HTMLButtonElement;
  copyButton: HTMLButtonElement;
  transcriptCounter: HTMLElement;
  transcriptMeta: HTMLElement;
  transcriptText: HTMLElement;
  answerSplitter: HTMLElement;
  questionRow: HTMLElement;
  questionCounter: HTMLElement;
  questionPrevButton: HTMLButtonElement;
  questionAutoButton: HTMLButtonElement;
  questionNextButton: HTMLButtonElement;
  questionText: HTMLElement;
  answerText: HTMLElement;
}

/** Resolves all DOM nodes required by the app. */
export function getRefs(): DomRefs {
  return {
    appShell: requireElement("ct-app"),
    appFooter: requireElement("appFooter"),
    statusRow: requireElement("statusRow"),
    statusText: requireElement("statusText"),
    limitRow: requireElement("limitRow"),
    chatgptLimitText: requireElement("chatgptLimitText"),
    deepgramLimitText: requireElement("deepgramLimitText"),
    accountLabel: requireElement("accountLabel"),
    controlsPanel: requireElement("controlsPanel"),
    deepgramLink: requireElement("deepgramLink"),
    developerLink: requireElement<HTMLButtonElement>("developerLink"),
    sourceLink: requireElement<HTMLButtonElement>("sourceLink"),
    apiKeyInput: requireElement<HTMLInputElement>("apiKeyInput"),
    testButton: requireElement<HTMLButtonElement>("testButton"),
    chatgptLoginButton: requireElement<HTMLButtonElement>("chatgptLoginButton"),
    chatgptSignOutButton: requireElement<HTMLButtonElement>("chatgptSignOutButton"),
    modelRefreshButton: requireElement<HTMLButtonElement>("modelRefreshButton"),
    cvFileInput: requireElement<HTMLInputElement>("cvFileInput"),
    uploadCvButton: requireElement<HTMLButtonElement>("uploadCvButton"),
    removeCvButton: requireElement<HTMLButtonElement>("removeCvButton"),
    cvStatus: requireElement("cvStatus"),
    speakerToggle: requireElement("speakerToggle"),
    microphoneToggle: requireElement("microphoneToggle"),
    speakerSelect: requireElement<HTMLSelectElement>("speakerSelect"),
    microphoneSelect: requireElement<HTMLSelectElement>("microphoneSelect"),
    languageSelect: requireElement<HTMLSelectElement>("languageSelect"),
    modelSelect: requireElement<HTMLSelectElement>("modelSelect"),
    thinkingSelect: requireElement<HTMLSelectElement>("thinkingSelect"),
    fastSelect: requireElement<HTMLSelectElement>("fastSelect"),
    verbositySelect: requireElement<HTMLSelectElement>("verbositySelect"),
    targetPositionInput: requireElement<HTMLInputElement>("targetPositionInput"),
    answerTypeSelect: requireElement<HTMLSelectElement>("answerTypeSelect"),
    previousButton: requireElement<HTMLButtonElement>("previousButton"),
    nextButton: requireElement<HTMLButtonElement>("nextButton"),
    newButton: requireElement<HTMLButtonElement>("newButton"),
    deleteButton: requireElement<HTMLButtonElement>("deleteButton"),
    answerButton: requireElement<HTMLButtonElement>("answerButton"),
    compactButton: requireElement<HTMLButtonElement>("compactButton"),
    alwaysOnTopButton: requireElement<HTMLButtonElement>("alwaysOnTopButton"),
    startButton: requireElement<HTMLButtonElement>("startButton"),
    stopButton: requireElement<HTMLButtonElement>("stopButton"),
    copyButton: requireElement<HTMLButtonElement>("copyButton"),
    transcriptCounter: requireElement("transcriptCounter"),
    transcriptMeta: requireElement("transcriptMeta"),
    transcriptText: requireElement("transcriptText"),
    answerSplitter: requireElement("answerSplitter"),
    questionRow: requireElement("questionRow"),
    questionCounter: requireElement("questionCounter"),
    questionPrevButton: requireElement<HTMLButtonElement>("questionPrevButton"),
    questionAutoButton: requireElement<HTMLButtonElement>("questionAutoButton"),
    questionNextButton: requireElement<HTMLButtonElement>("questionNextButton"),
    questionText: requireElement("questionText"),
    answerText: requireElement("answerText"),
  };
}

/** Returns one required DOM element or fails startup loudly. */
function requireElement<T extends HTMLElement = HTMLElement>(id: string): T {
  const element = document.getElementById(id);
  if (!element) {
    throw new Error(`Missing DOM element: ${id}`);
  }
  return element as T;
}
