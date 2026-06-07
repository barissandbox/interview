"use strict";
/** Typed DOM lookup for the Interview UI. */
var InterviewDom;
(function (InterviewDom) {
    function getRefs() {
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
            developerLink: requireElement("developerLink"),
            sourceLink: requireElement("sourceLink"),
            apiKeyInput: requireElement("apiKeyInput"),
            testButton: requireElement("testButton"),
            chatgptLoginButton: requireElement("chatgptLoginButton"),
            chatgptSignOutButton: requireElement("chatgptSignOutButton"),
            modelRefreshButton: requireElement("modelRefreshButton"),
            cvFileInput: requireElement("cvFileInput"),
            uploadCvButton: requireElement("uploadCvButton"),
            removeCvButton: requireElement("removeCvButton"),
            cvStatus: requireElement("cvStatus"),
            speakerToggle: requireElement("speakerToggle"),
            microphoneToggle: requireElement("microphoneToggle"),
            speakerSelect: requireElement("speakerSelect"),
            microphoneSelect: requireElement("microphoneSelect"),
            languageSelect: requireElement("languageSelect"),
            modelSelect: requireElement("modelSelect"),
            thinkingSelect: requireElement("thinkingSelect"),
            targetPositionInput: requireElement("targetPositionInput"),
            answerTypeSelect: requireElement("answerTypeSelect"),
            previousButton: requireElement("previousButton"),
            nextButton: requireElement("nextButton"),
            newButton: requireElement("newButton"),
            deleteButton: requireElement("deleteButton"),
            answerButton: requireElement("answerButton"),
            compactButton: requireElement("compactButton"),
            startButton: requireElement("startButton"),
            stopButton: requireElement("stopButton"),
            copyButton: requireElement("copyButton"),
            transcriptCounter: requireElement("transcriptCounter"),
            transcriptMeta: requireElement("transcriptMeta"),
            transcriptText: requireElement("transcriptText"),
            answerSplitter: requireElement("answerSplitter"),
            questionRow: requireElement("questionRow"),
            questionCounter: requireElement("questionCounter"),
            questionPrevButton: requireElement("questionPrevButton"),
            questionAutoButton: requireElement("questionAutoButton"),
            questionNextButton: requireElement("questionNextButton"),
            questionText: requireElement("questionText"),
            answerText: requireElement("answerText"),
        };
    }
    InterviewDom.getRefs = getRefs;
    function requireElement(id) {
        const element = document.getElementById(id);
        if (!element) {
            throw new Error(`Missing DOM element: ${id}`);
        }
        return element;
    }
})(InterviewDom || (InterviewDom = {}));
/** Tauri backend access for the Interview UI. */
var InterviewBackend;
(function (InterviewBackend) {
    const { invoke } = window.__TAURI__.core;
    const { listen } = window.__TAURI__.event;
    function invokeCommand(command, args) {
        return invoke(command, args);
    }
    InterviewBackend.invokeCommand = invokeCommand;
    function listenTranscriptEvents(handler) {
        return listen("transcript-event", (event) => {
            handler(event.payload);
        });
    }
    InterviewBackend.listenTranscriptEvents = listenTranscriptEvents;
})(InterviewBackend || (InterviewBackend = {}));
/** Rendering and UI state helpers for Interview. */
var InterviewRender;
(function (InterviewRender) {
    /** Collects current UI settings into the FrontendSettings shape. */
    function collectSettings(refs) {
        return {
            speakerDeviceId: refs.speakerSelect.value,
            microphoneDeviceId: refs.microphoneSelect.value,
            language: refs.languageSelect.value,
            speakerEnabled: refs.speakerToggle.classList.contains("is-on"),
            microphoneEnabled: refs.microphoneToggle.classList.contains("is-on"),
            model: refs.modelSelect.value,
            thinkingVariant: refs.thinkingSelect.value,
            answerType: refs.answerTypeSelect.value,
            targetPosition: refs.targetPositionInput.value,
        };
    }
    InterviewRender.collectSettings = collectSettings;
    /** Renders all UI fragments from state. */
    function renderState(refs, model, state) {
        const previousQuestionCount = model.lastQuestionCount;
        const previousState = model.appState;
        model.appState = state;
        const pendingIndex = latestPendingQuestionIndex(state, previousState);
        if (pendingIndex >= 0 && (model.autoFollowQuestions || model.selectedQuestionIndex < 0)) {
            model.selectedQuestionIndex = pendingIndex;
        }
        else if (model.autoFollowQuestions && state.questions.length > previousQuestionCount) {
            model.selectedQuestionIndex = state.questions.length - 1;
        }
        else {
            model.selectedQuestionIndex = normalizeQuestionIndex(model.selectedQuestionIndex, state.questions.length, state.selectedQuestionIndex);
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
    InterviewRender.renderState = renderState;
    /** Renders current status text. */
    function renderStatus(refs, message, isError = false) {
        refs.statusText.textContent = message || "Ready.";
        refs.statusRow.classList.toggle("is-error", isError);
    }
    InterviewRender.renderStatus = renderStatus;
    /** Renders account and balance data in the dedicated limit row. */
    function renderLimits(refs, state) {
        const account = state.chatgpt.loggedIn ? state.chatgpt.accountEmail.trim() : "";
        refs.chatgptLimitText.textContent = [account, state.chatgpt.limitLabel]
            .filter(Boolean)
            .join(" ") || "--";
        refs.deepgramLimitText.textContent = formatDeepgramLimit(state.balance, state.settings.apiKey);
    }
    InterviewRender.renderLimits = renderLimits;
    /** Applies a streaming answer event without waiting for full state refresh. */
    function renderStreamingAnswer(refs, model, questionId, question, answer) {
        model.streamingQuestionId = questionId;
        model.streamingAnswer = answer;
        const index = model.appState?.questions.findIndex((item) => item.id === questionId) ?? -1;
        if (index >= 0 && (model.autoFollowQuestions || model.selectedQuestionIndex < 0)) {
            model.selectedQuestionIndex = index;
        }
        if (index >= 0 && model.selectedQuestionIndex === index) {
            renderAnswer(refs, model);
        }
        else if (!model.appState || (index < 0 && model.autoFollowQuestions)) {
            refs.questionRow.hidden = false;
            refs.questionText.textContent = question;
            renderAnswerMarkdown(refs.answerText, answer || pendingDots(model), !answer);
            refs.answerText.scrollTop = refs.answerText.scrollHeight;
        }
        renderStatus(refs, "Streaming answer...");
        updateButtons(refs, model);
    }
    InterviewRender.renderStreamingAnswer = renderStreamingAnswer;
    /** Shows a temporary copied state on the copy button. */
    function renderCopyFeedback(refs, model) {
        window.clearTimeout(model.copyResetTimer);
        refs.copyButton.classList.add("is-copied");
        refs.copyButton.textContent = "copied";
        model.copyResetTimer = window.setTimeout(() => {
            refs.copyButton.classList.remove("is-copied");
            refs.copyButton.textContent = "copy";
            updateButtons(refs, model);
        }, 1200);
    }
    InterviewRender.renderCopyFeedback = renderCopyFeedback;
    /** Builds clipboard text with transcript, question, and answer. */
    function getCopyText(refs) {
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
    InterviewRender.getCopyText = getCopyText;
    /** Renders saved and interim transcript text. */
    function renderTranscript(refs, model) {
        const parts = [];
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
    InterviewRender.renderTranscript = renderTranscript;
    /** Renders selected question and answer text. */
    function renderAnswer(refs, model) {
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
        renderAnswerMarkdown(refs.answerText, answer || (selected.pending ? pendingDots(model) : "No answer yet."), !answer);
        refs.answerText.scrollTop = refs.answerText.scrollHeight;
    }
    InterviewRender.renderAnswer = renderAnswer;
    /** Moves the selected question by offset. */
    function selectQuestion(refs, model, offset) {
        const count = model.appState?.questions.length ?? 0;
        if (count === 0) {
            model.selectedQuestionIndex = -1;
        }
        else {
            model.selectedQuestionIndex = Math.max(0, Math.min(count - 1, model.selectedQuestionIndex + offset));
        }
        renderAnswer(refs, model);
        updateButtons(refs, model);
    }
    InterviewRender.selectQuestion = selectQuestion;
    /** Updates button visibility and disabled states. */
    function updateButtons(refs, model) {
        if (!model.appState)
            return;
        const running = Boolean(model.appState.running);
        const apiKeyReady = isApiKeyReady(refs, model);
        const chatgptReady = Boolean(model.appState.chatgpt.loggedIn);
        const hasSelectedText = Boolean(selectedAnswerText());
        const sourceReady = refs.speakerToggle.classList.contains("is-on") ||
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
        refs.modelRefreshButton.hidden = !chatgptReady;
        refs.modelRefreshButton.disabled = running || !chatgptReady;
        refs.uploadCvButton.disabled = running;
        refs.answerButton.disabled =
            !chatgptReady || Boolean(model.appState.answerPending) || !hasSelectedText;
        refs.answerButton.title = hasSelectedText ? "Generate answer" : "Select text first.";
        refs.compactButton.textContent = model.compactMode ? "Full" : "Compact";
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
    InterviewRender.updateButtons = updateButtons;
    /** Populates device, language, model, and thinking selects. */
    function populateOptions(refs, model) {
        if (!model.appState)
            return;
        replaceOptions(refs.languageSelect, model.appState.languages.map((item) => ({
            value: item.value,
            label: item.label,
        })));
        replaceOptions(refs.modelSelect, model.appState.models
            .slice()
            .filter((item) => !item.hidden && item.inputModalities.includes("text"))
            .sort(compareModels)
            .map((item) => ({
            value: item.model,
            label: item.displayName || item.model,
            title: item.description || item.model,
        })));
        replaceOptions(refs.thinkingSelect, model.appState.thinkingVariants.map((item) => ({
            value: item.value,
            label: item.value,
            title: item.description,
        })));
        replaceOptions(refs.speakerSelect, model.appState.devices
            .filter((item) => item.kind === "Speaker")
            .map(deviceOption));
        replaceOptions(refs.microphoneSelect, model.appState.devices
            .filter((item) => item.kind === "Microphone")
            .map(deviceOption));
        refs.apiKeyInput.value = model.appState.settings.apiKey || "";
        selectPreferredDevice(refs.speakerSelect, model.appState.settings.speakerDeviceId || "");
        selectPreferredDevice(refs.microphoneSelect, model.appState.settings.microphoneDeviceId || "");
        refs.languageSelect.value = model.appState.settings.language || "en-US";
        refs.modelSelect.value = model.appState.settings.model || "gpt-5.4-mini";
        refs.thinkingSelect.value = model.appState.settings.thinkingVariant || "low";
        refs.answerTypeSelect.value = model.appState.settings.answerType || "details";
        refs.targetPositionInput.value = model.appState.settings.targetPosition || "";
    }
    function renderAccount(refs, _state) {
        refs.accountLabel.hidden = true;
        refs.accountLabel.textContent = "";
    }
    function setCompactMode(refs, model, enabled) {
        model.compactMode = enabled;
        refs.appShell.classList.toggle("is-compact", enabled);
        updateButtons(refs, model);
    }
    InterviewRender.setCompactMode = setCompactMode;
    function setAutoFollowQuestions(refs, model, enabled) {
        model.autoFollowQuestions = enabled;
        if (enabled && model.appState?.questions.length) {
            model.selectedQuestionIndex = model.appState.questions.length - 1;
            renderAnswer(refs, model);
        }
        updateButtons(refs, model);
    }
    InterviewRender.setAutoFollowQuestions = setAutoFollowQuestions;
    function renderProfile(refs, state) {
        if (!state.profile.textLength) {
            refs.cvStatus.textContent = "No CV loaded.";
            return;
        }
        refs.cvStatus.textContent = `${state.profile.fileName || "CV"} loaded locally (${state.profile.textLength} chars).`;
    }
    /** Selects a saved device or falls back to the first option. */
    function selectPreferredDevice(select, savedValue) {
        if (savedValue &&
            Array.from(select.options).some((option) => option.value === savedValue)) {
            select.value = savedValue;
            return;
        }
        if (select.options.length > 0) {
            select.selectedIndex = 0;
        }
    }
    /** Replaces select options without preserving stale values. */
    function replaceOptions(select, options) {
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
    function deviceOption(device) {
        const suffix = device.isDefault ? " (Default)" : "";
        const label = cleanDeviceName(device.name) || device.name || device.id;
        return {
            value: device.id,
            label: `${label}${suffix}`,
            title: `${device.name}${suffix}`,
        };
    }
    /** Removes noisy Windows endpoint suffixes from device names. */
    function cleanDeviceName(name) {
        return String(name || "")
            .replace(/\s*\((?:Realtek\(R\)|NVIDIA|AMD|Intel\(R\)|High Definition Audio Device|USB Audio Device)[^)]+\)/gi, "")
            .replace(/\s{2,}/g, " ")
            .trim();
    }
    /** Renders toggle state. */
    function renderToggles(refs, model) {
        if (!model.appState)
            return;
        setToggle(refs.speakerToggle, Boolean(model.appState.settings.speakerEnabled));
        setToggle(refs.microphoneToggle, Boolean(model.appState.settings.microphoneEnabled));
    }
    /** Sets one switch visual state. */
    function setToggle(element, enabled) {
        element.classList.toggle("is-on", enabled);
        element.setAttribute("aria-pressed", String(enabled));
    }
    /** Renders transcript header metadata. */
    function renderTranscriptHeader(refs, model) {
        if (!model.appState)
            return;
        const count = model.appState.transcriptCount || 0;
        const index = count ? model.appState.activeIndex + 1 : 0;
        refs.transcriptCounter.textContent = `${index}/${count}`;
        const active = model.appState.transcripts[model.appState.activeIndex];
        refs.transcriptMeta.textContent = active
            ? active.label
            : "No transcript selected";
    }
    /** Returns true when the visible key matches the last tested and saved key. */
    function isApiKeyReady(refs, model) {
        const saved = model.appState?.settings?.apiKey || "";
        return Boolean(saved.trim()) && refs.apiKeyInput.value.trim() === saved.trim();
    }
    /** Returns true when the only transcript has no saved text or questions. */
    function isLastEmptyTranscript(model) {
        const state = model.appState;
        if (!state)
            return false;
        return (state.transcriptCount === 1 &&
            !state.transcriptText.trim() &&
            state.questions.length === 0);
    }
    function normalizeQuestionIndex(current, count, fallback) {
        if (count <= 0)
            return -1;
        if (current >= 0 && current < count)
            return current;
        if (fallback >= 0 && fallback < count)
            return fallback;
        return count - 1;
    }
    function formatDeepgramLimit(balance, apiKey) {
        if (!balance) {
            return apiKey ? "Deepgram: --" : "Deepgram: --";
        }
        return balance.replace(/^Deepgram:\s*\$(\d+(?:\.\d+)?)/i, "Deepgram: $1$");
    }
    function selectedAnswerText() {
        return window.getSelection()?.toString().trim() || "";
    }
    function latestPendingQuestionIndex(state, previousState) {
        for (let index = state.questions.length - 1; index >= 0; index -= 1) {
            const question = state.questions[index];
            if (!question?.pending)
                continue;
            const previous = previousState?.questions.find((item) => item.id === question.id);
            if (!previous || !previous.pending) {
                return index;
            }
        }
        return -1;
    }
    function renderAnswerMarkdown(container, markdown, empty) {
        container.classList.toggle("ct-empty", empty);
        container.innerHTML = "";
        const text = markdown.trim();
        if (!text)
            return;
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
    function startPendingAnswerAnimation(refs, model) {
        if (model.pendingAnswerTimer)
            return;
        model.pendingAnswerTimer = window.setInterval(() => {
            if (!model.appState?.answerPending) {
                stopPendingAnswerAnimation(model);
                return;
            }
            model.pendingAnswerTick = (model.pendingAnswerTick + 1) % 3;
            renderAnswer(refs, model);
        }, 200);
    }
    InterviewRender.startPendingAnswerAnimation = startPendingAnswerAnimation;
    function stopPendingAnswerAnimation(model) {
        window.clearInterval(model.pendingAnswerTimer);
        model.pendingAnswerTimer = undefined;
        model.pendingAnswerTick = 0;
    }
    InterviewRender.stopPendingAnswerAnimation = stopPendingAnswerAnimation;
    function pendingDots(model) {
        return ".".repeat((model.pendingAnswerTick % 3) + 1);
    }
    function appendInlineMarkdown(parent, text) {
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
    function compareModels(a, b) {
        const left = modelSortParts(a.model || a.displayName);
        const right = modelSortParts(b.model || b.displayName);
        for (let index = 0; index < Math.max(left.numbers.length, right.numbers.length); index += 1) {
            const diff = (right.numbers[index] || 0) - (left.numbers[index] || 0);
            if (diff !== 0)
                return diff;
        }
        if (left.mini !== right.mini)
            return left.mini ? 1 : -1;
        return (a.displayName || a.model).localeCompare(b.displayName || b.model);
    }
    function modelSortParts(value) {
        const numbers = (String(value).match(/\d+(?:\.\d+)?/g) || []).map(Number);
        return {
            numbers,
            mini: /\bmini\b/i.test(value),
        };
    }
})(InterviewRender || (InterviewRender = {}));
/** Tauri frontend for Interview controls and transcript rendering. */
var InterviewApp;
(function (InterviewApp) {
    const refs = InterviewDom.getRefs();
    const model = {
        appState: null,
        autoFollowQuestions: true,
        compactHeightDelta: 0,
        compactMode: false,
        interimText: "",
        lastQuestionCount: 0,
        selectedQuestionIndex: -1,
        streamingAnswer: "",
        streamingQuestionId: "",
        pendingAnswerTick: 0,
    };
    let lastBalanceRefreshKey = "";
    let chatgptLimitRefreshAttempted = false;
    document.addEventListener("DOMContentLoaded", async () => {
        bindEvents();
        bindSplitter();
        await bindBackendEvents();
        await refreshState();
    });
    /** Wires UI controls to Tauri commands. */
    function bindEvents() {
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
            const state = await safeInvoke("test_deepgram_key", {
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
            const state = await safeInvoke("start_chatgpt_login");
            if (state) {
                renderStateAndUpdate(state);
                void refreshDeepgramBalanceIfNeeded(state);
            }
        });
        refs.chatgptSignOutButton.addEventListener("click", async () => {
            const state = await safeInvoke("sign_out_chatgpt");
            if (state)
                renderStateAndUpdate(state);
        });
        refs.modelRefreshButton.addEventListener("click", async () => {
            const state = await safeInvoke("refresh_chatgpt_models");
            if (state)
                renderStateAndUpdate(state);
        });
        refs.uploadCvButton.addEventListener("click", () => refs.cvFileInput.click());
        refs.cvFileInput.addEventListener("change", () => {
            void uploadCvProfile();
        });
        refs.removeCvButton.addEventListener("click", async () => {
            const state = await safeInvoke("remove_cv_profile");
            if (state)
                renderStateAndUpdate(state);
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
        });
        for (const element of [
            refs.speakerSelect,
            refs.microphoneSelect,
            refs.languageSelect,
            refs.modelSelect,
            refs.thinkingSelect,
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
        refs.newButton.addEventListener("click", async () => {
            model.interimText = "";
            model.lastQuestionCount = 0;
            model.selectedQuestionIndex = -1;
            const state = await safeInvoke("create_transcript");
            if (state)
                renderStateAndUpdate(state);
        });
        refs.deleteButton.addEventListener("click", async () => {
            model.interimText = "";
            model.lastQuestionCount = 0;
            model.selectedQuestionIndex = -1;
            const state = await safeInvoke("delete_transcript");
            if (state)
                renderStateAndUpdate(state);
        });
        refs.answerButton.addEventListener("click", async () => {
            await saveSettingsOnly();
            const state = await safeInvoke("generate_answer", {
                question: selectedAnswerInput(),
            });
            if (state)
                renderStateAndUpdate(state);
        });
        refs.startButton.addEventListener("click", async () => {
            await saveSettingsOnly();
            model.interimText = "";
            const state = await safeInvoke("start_capture");
            if (state)
                renderStateAndUpdate(state);
        });
        refs.stopButton.addEventListener("click", async () => {
            model.interimText = "";
            const state = await safeInvoke("stop_capture");
            if (state)
                renderStateAndUpdate(state);
        });
        refs.copyButton.addEventListener("click", async () => {
            const text = InterviewRender.getCopyText(refs);
            if (!text)
                return;
            try {
                await navigator.clipboard.writeText(text);
                InterviewRender.renderCopyFeedback(refs, model);
                InterviewRender.renderStatus(refs, "Copied.");
            }
            catch (error) {
                InterviewRender.renderStatus(refs, `Copy failed: ${error}`, true);
            }
        });
    }
    /** Enables vertical resizing between transcript and answer panes. */
    function bindSplitter() {
        const bodyElement = refs.answerSplitter.parentElement;
        if (!bodyElement)
            return;
        refs.answerSplitter.addEventListener("pointerdown", (event) => {
            event.preventDefault();
            refs.answerSplitter.setPointerCapture(event.pointerId);
            bodyElement.classList.add("is-resizing");
            const handleMove = (moveEvent) => {
                const rect = bodyElement.getBoundingClientRect();
                const minPaneHeight = 72;
                const topHeight = Math.max(minPaneHeight, Math.min(rect.height - 120 - refs.answerSplitter.offsetHeight, moveEvent.clientY - rect.top));
                bodyElement.style.setProperty("--ct-top-pane-size", `${topHeight}px`);
            };
            const stopResize = () => {
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
    async function bindBackendEvents() {
        await InterviewBackend.listenTranscriptEvents((payload) => {
            if (payload.type === "status") {
                InterviewRender.renderStatus(refs, payload.message);
            }
            else if (payload.type === "interim") {
                model.interimText = payload.text || "";
                InterviewRender.renderTranscript(refs, model);
            }
            else if (payload.type === "answer") {
                InterviewRender.renderStreamingAnswer(refs, model, payload.questionId, payload.question, payload.answer);
            }
            else if (payload.type === "state") {
                model.interimText = "";
                renderStateAndUpdate(payload.state);
                void refreshDeepgramBalanceIfNeeded(payload.state);
            }
            else if (payload.type === "error") {
                InterviewRender.renderStatus(refs, payload.message, true);
            }
        });
    }
    /** Safely invokes a Tauri command, catching and displaying errors. */
    async function safeInvoke(command, args) {
        try {
            return await InterviewBackend.invokeCommand(command, args);
        }
        catch (error) {
            InterviewRender.renderStatus(refs, String(error), true);
            InterviewRender.updateButtons(refs, model);
            return null;
        }
    }
    /** Fetches current state from Rust. */
    async function refreshState() {
        const state = await safeInvoke("get_app_state");
        if (state) {
            renderStateAndUpdate(state);
            void refreshDeepgramBalanceIfNeeded(state);
            void refreshChatgptLimitsIfStale(state);
        }
    }
    async function refreshDeepgramBalanceIfNeeded(state) {
        const apiKey = state.settings.apiKey.trim();
        if (!apiKey || state.balance || lastBalanceRefreshKey === apiKey)
            return;
        lastBalanceRefreshKey = apiKey;
        const refreshed = await safeInvoke("test_deepgram_key", { apiKey });
        if (refreshed) {
            renderStateAndUpdate({
                ...refreshed,
                status: state.status || "Ready.",
            });
        }
    }
    async function refreshChatgptLimitsIfStale(state) {
        if (chatgptLimitRefreshAttempted ||
            !state.chatgpt.loggedIn ||
            /\bresets\b/i.test(state.chatgpt.limitLabel)) {
            return;
        }
        chatgptLimitRefreshAttempted = true;
        try {
            const refreshed = await InterviewBackend.invokeCommand("refresh_chatgpt_models");
            renderStateAndUpdate(refreshed);
        }
        catch {
            // Keep startup quiet when the usage endpoint is temporarily unavailable.
        }
    }
    async function toggleCompactMode() {
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
    function nextAnimationFrame() {
        return new Promise((resolve) => window.requestAnimationFrame(() => resolve()));
    }
    /** Navigates to a transcript by offset. */
    async function navigateTranscript(offset) {
        model.interimText = "";
        model.lastQuestionCount = 0;
        model.selectedQuestionIndex = -1;
        const state = await safeInvoke("select_transcript_by_offset", {
            offset,
        });
        if (state)
            renderStateAndUpdate(state);
    }
    /** Saves settings and renders returned state. */
    async function saveSettingsAndRender() {
        const state = await safeInvoke("save_settings", {
            settings: InterviewRender.collectSettings(refs),
        });
        if (state) {
            renderStateAndUpdate(state);
        }
    }
    /** Saves settings without rerendering the whole tree. */
    async function saveSettingsOnly() {
        await safeInvoke("save_settings", {
            settings: InterviewRender.collectSettings(refs),
        });
    }
    /** Uploads a selected PDF CV to the backend. */
    async function uploadCvProfile() {
        const file = refs.cvFileInput.files?.[0];
        refs.cvFileInput.value = "";
        if (!file)
            return;
        const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
        const state = await safeInvoke("upload_cv_profile", {
            fileName: file.name,
            bytes,
        });
        if (state)
            renderStateAndUpdate(state);
    }
    function updatePendingAnswerAnimation() {
        if (model.appState?.answerPending) {
            InterviewRender.startPendingAnswerAnimation(refs, model);
        }
        else {
            InterviewRender.stopPendingAnswerAnimation(model);
        }
    }
    function renderStateAndUpdate(state) {
        InterviewRender.renderState(refs, model, state);
        updatePendingAnswerAnimation();
    }
    /** Returns selected text from the page, constrained for answer generation. */
    function selectedAnswerInput() {
        return window.getSelection()?.toString().trim().slice(0, 2000) || "";
    }
})(InterviewApp || (InterviewApp = {}));
