/**
 * Semantic Chat UI — talks to the local Axum API.
 * UI chats (/api/chats, /api/messages) are separate from remote Chat Completions.
 */

const MESSAGE_TIMEOUT_MS = 60_000;

const viewStart = document.getElementById("view-start");
const viewApp = document.getElementById("view-app");
const startForm = document.getElementById("start-form");
const startInput = document.getElementById("start-input");
const newChatBtn = document.getElementById("new-chat-btn");
const chatListEl = document.getElementById("chat-list");
const chatTitleEl = document.getElementById("chat-title");
const messageListEl = document.getElementById("message-list");
const composerForm = document.getElementById("composer-form");
const composerInput = document.getElementById("composer-input");

/** @type {string | null} */
let activeChatId = null;

/** @type {Array<{id: string, title: string, created_at: string}>} */
let chatSummaries = [];

let busy = false;

function showStartView() {
  activeChatId = null;
  viewStart.classList.remove("is-hidden");
  viewApp.classList.add("is-hidden");
  startInput.value = "";
  startInput.focus();
}

function showAppView() {
  viewStart.classList.add("is-hidden");
  viewApp.classList.remove("is-hidden");
  composerInput.focus();
}

function setBusy(isBusy) {
  busy = isBusy;
  startInput.disabled = isBusy;
  composerInput.disabled = isBusy;
  newChatBtn.disabled = isBusy;
  const submitButtons = document.querySelectorAll(
    '#start-form button[type="submit"], #composer-form button[type="submit"]'
  );
  for (const button of submitButtons) {
    button.disabled = isBusy;
  }
}

function setChatTitle(title) {
  chatTitleEl.textContent = title || "New chat";
}

function clearMessages() {
  messageListEl.innerHTML = "";
}

function appendMessage(role, content, options = {}) {
  const bubble = document.createElement("article");
  const normalized = String(role).toLowerCase();
  bubble.className =
    normalized === "user" ? "message message-user" : "message message-assistant";
  if (options.error) {
    bubble.classList.add("message-error");
  }

  const meta = document.createElement("span");
  meta.className = "message-meta";
  meta.textContent = normalized === "user" ? "You" : "Assistant";
  if (options.cacheHit) {
    meta.textContent = "Assistant · cached";
  }

  const body = document.createElement("div");
  body.textContent = content;

  bubble.appendChild(meta);
  bubble.appendChild(body);
  messageListEl.appendChild(bubble);
  messageListEl.scrollTop = messageListEl.scrollHeight;
  return bubble;
}

function showTypingIndicator() {
  const bubble = document.createElement("article");
  bubble.className = "message message-assistant message-pending";
  bubble.dataset.typing = "true";

  const meta = document.createElement("span");
  meta.className = "message-meta";
  meta.textContent = "Assistant";

  const dots = document.createElement("div");
  dots.className = "typing-dots";
  dots.setAttribute("aria-label", "Waiting for reply");
  dots.innerHTML = "<span></span><span></span><span></span>";

  bubble.appendChild(meta);
  bubble.appendChild(dots);
  messageListEl.appendChild(bubble);
  messageListEl.scrollTop = messageListEl.scrollHeight;
  return bubble;
}

function removeTypingIndicator(bubble) {
  if (bubble && bubble.parentNode) {
    bubble.parentNode.removeChild(bubble);
  }
}

function renderMessages(messages) {
  clearMessages();
  for (const message of messages || []) {
    appendMessage(message.role, message.content);
  }
}

function renderChatList() {
  chatListEl.innerHTML = "";
  if (!chatSummaries.length) {
    const empty = document.createElement("li");
    empty.className = "chat-list-empty";
    empty.textContent = "No chats yet";
    chatListEl.appendChild(empty);
    return;
  }

  for (const chat of chatSummaries) {
    const li = document.createElement("li");
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "chat-list-item";
    if (chat.id === activeChatId) {
      btn.classList.add("is-active");
    }
    btn.textContent = chat.title || "New chat";
    btn.addEventListener("click", () => {
      void selectChat(chat.id);
    });
    li.appendChild(btn);
    chatListEl.appendChild(li);
  }
}

function isAbortError(error) {
  return (
    error?.name === "AbortError" ||
    (typeof error?.message === "string" &&
      error.message.toLowerCase().includes("aborted"))
  );
}

async function api(path, options = {}) {
  const headers = { ...(options.headers || {}) };
  if (options.body && !headers["Content-Type"]) {
    headers["Content-Type"] = "application/json";
  }

  const timeoutMs = options.timeoutMs;
  const controller = timeoutMs ? new AbortController() : null;
  let timeoutId = null;
  if (controller) {
    timeoutId = window.setTimeout(() => controller.abort(), timeoutMs);
  }

  try {
    const response = await fetch(path, {
      ...options,
      headers,
      signal: controller ? controller.signal : options.signal,
    });
    if (!response.ok) {
      const detail = await response.text();
      throw new Error(detail || `Request failed (${response.status})`);
    }

    if (response.status === 204) {
      return null;
    }

    const text = await response.text();
    if (!text) {
      return null;
    }
    return JSON.parse(text);
  } finally {
    if (timeoutId !== null) {
      window.clearTimeout(timeoutId);
    }
  }
}

async function refreshChatList() {
  chatSummaries = await api("/api/chats");
  renderChatList();
}

async function selectChat(chatId) {
  if (busy) {
    return;
  }

  setBusy(true);
  try {
    const session = await api(`/api/chats/${chatId}`);
    activeChatId = session.id;
    setChatTitle(session.title);
    renderMessages(session.messages);
    renderChatList();
    showAppView();
  } catch (error) {
    console.error(error);
    window.alert(`Could not load chat: ${error.message}`);
  } finally {
    setBusy(false);
  }
}

async function createChat() {
  return api("/api/chats", { method: "POST", body: "{}" });
}

async function sendMessage(chatId, message) {
  return api("/api/messages", {
    method: "POST",
    body: JSON.stringify({ chat_id: chatId, message }),
    timeoutMs: MESSAGE_TIMEOUT_MS,
  });
}

async function sendAndShowReply(chatId, text) {
  const typing = showTypingIndicator();
  try {
    const result = await sendMessage(chatId, text);
    removeTypingIndicator(typing);
    appendMessage("assistant", result.reply, { cacheHit: result.cache_hit });
    setChatTitle(result.title);
    await refreshChatList();
    return result;
  } catch (error) {
    removeTypingIndicator(typing);
    if (isAbortError(error)) {
      appendMessage("assistant", "timeout. try again.", { error: true });
      return null;
    }
    throw error;
  }
}

startForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (busy) {
    return;
  }

  const text = startInput.value.trim();
  if (!text) {
    return;
  }

  setBusy(true);
  try {
    const created = await createChat();
    activeChatId = created.id;
    showAppView();
    setChatTitle(created.title);
    clearMessages();
    appendMessage("user", text);

    await sendAndShowReply(created.id, text);
    startInput.value = "";
    composerInput.value = "";
  } catch (error) {
    console.error(error);
    window.alert(`Could not start chat: ${error.message}`);
  } finally {
    setBusy(false);
    if (activeChatId) {
      composerInput.focus();
    } else {
      startInput.focus();
    }
  }
});

newChatBtn.addEventListener("click", async () => {
  if (busy) {
    return;
  }

  setBusy(true);
  try {
    const created = await createChat();
    activeChatId = created.id;
    setChatTitle(created.title);
    clearMessages();
    await refreshChatList();
    showAppView();
    composerInput.focus();
  } catch (error) {
    console.error(error);
    window.alert(`Could not create chat: ${error.message}`);
  } finally {
    setBusy(false);
  }
});

composerForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (busy) {
    return;
  }

  const text = composerInput.value.trim();
  if (!text) {
    return;
  }

  if (!activeChatId) {
    window.alert("Create or select a chat first.");
    return;
  }

  setBusy(true);
  try {
    appendMessage("user", text);
    composerInput.value = "";
    await sendAndShowReply(activeChatId, text);
  } catch (error) {
    console.error(error);
    window.alert(`Could not send message: ${error.message}`);
  } finally {
    setBusy(false);
    composerInput.focus();
  }
});

void (async function init() {
  try {
    await refreshChatList();
  } catch (error) {
    console.error("Failed to load chats:", error);
    renderChatList();
  }
  showStartView();
})();
