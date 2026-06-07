/** Shared type definitions mirroring Rust backend view models. */

export interface AppSettings {
  apiKey: string;
  speakerDeviceId: string;
  microphoneDeviceId: string;
  language: string;
  speakerEnabled: boolean;
  microphoneEnabled: boolean;
  activeTranscriptId: string;
  model: string;
  thinkingVariant: string;
  answerType: string;
  fastEnabled: boolean;
  verbosity: string;
  targetPosition: string;
  alwaysOnTop: boolean;
}

export interface TranscriptSummary {
  id: string;
  label: string;
}

export interface AudioDevice {
  id: string;
  name: string;
  kind: "Speaker" | "Microphone";
  isDefault: boolean;
  isAvailable: boolean;
}

export interface LanguageOption {
  value: string;
  label: string;
  model: string | null;
}

export interface ThinkingVariantOption {
  value: string;
  description: string;
}

export interface AvailableModel {
  id: string;
  model: string;
  displayName: string;
  description: string;
  hidden: boolean;
  isDefault: boolean;
  inputModalities: string[];
  defaultThinkingVariant: string;
  thinkingVariants: ThinkingVariantOption[];
}

export interface QuestionRecord {
  id: string;
  key: string;
  text: string;
  answer: string;
  pending: boolean;
  answerType: string;
  model: string;
  thinkingVariant: string;
  createdAt: string;
  updatedAt: string;
}

export interface ChatGptViewState {
  loggedIn: boolean;
  accountEmail: string;
  limitLabel: string;
  error: string;
}

export interface ProfileViewState {
  fileName: string;
  textLength: number;
  updatedAt: string | null;
}

export interface AppViewState {
  settings: AppSettings;
  balance: string;
  status: string;
  chatgpt: ChatGptViewState;
  profile: ProfileViewState;
  models: AvailableModel[];
  thinkingVariants: ThinkingVariantOption[];
  transcripts: TranscriptSummary[];
  activeTranscriptId: string;
  activeIndex: number;
  transcriptCount: number;
  transcriptText: string;
  questions: QuestionRecord[];
  selectedQuestionIndex: number;
  selectedQuestion: string;
  answerText: string;
  answerPending: boolean;
  devices: AudioDevice[];
  languages: LanguageOption[];
  running: boolean;
}

export interface FrontendSettings {
  speakerDeviceId: string;
  microphoneDeviceId: string;
  language: string;
  speakerEnabled: boolean;
  microphoneEnabled: boolean;
  model: string;
  thinkingVariant: string;
  answerType: string;
  fastEnabled: boolean;
  verbosity: string;
  targetPosition: string;
  alwaysOnTop: boolean;
}

export interface SelectOption {
  value: string;
  label: string;
  title?: string;
}

export type UiEventPayload =
  | { type: "status"; message: string }
  | { type: "interim"; text: string }
  | { type: "state"; state: AppViewState }
  | { type: "answer"; questionId: string; question: string; answer: string; streaming: boolean }
  | { type: "error"; message: string };
