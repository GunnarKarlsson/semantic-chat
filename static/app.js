/**
 * Frontend shell for Semantic Chat.
 * Step 7: structure + view switching.
 * Step 8 will wire the API calls.
 */

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

/** @type {"start" | "app"} */
let currentView = "start";

/** @type {string | null} */
let activeChatId = null;

function showStartView() {
  currentView = "start";
  activeChatId = null;
  viewStart.classList.remove("is-hidden");
  viewApp.classList.add("is-hidden");
  startInput.value = "";
  startInput.focus();
}

function showAppView() {
  currentView = "app";
  viewStart.classList.add("is-hidden");
  viewApp.classList.remove("is-hidden");
  composerInput.focus();
}

function setChatTitle(title) {
  chatTitleEl.textContent = title || "New chat";
}

function clearMessages() {
  messageListEl.innerHTML = "";
}

function renderEmptyChatList() {
  chatListEl.innerHTML = "";
  const empty = document.createElement("li");
  empty.className = "chat-list-empty";
  empty.textContent = "No chats yet";
  chatListEl.appendChild(empty);
}

function renderChatList(chats, activeId) {
  chatListEl.innerHTML = "";
  if (!chats || chats.length === 0) {
    renderEmptyChatList();
    return;
  }

  for (const chat of chats) {
    const li = document.createElement("li");
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "chat-list-item";
    if (chat.id === activeId) {
      btn.classList.add("is-active");
    }
    btn.textContent = chat.title || "New chat";
    btn.dataset.chatId = chat.id;
    btn.addEventListener("click", () => {
      // Step 8: load chat via GET /api/chats/:id
      activeChatId = chat.id;
      setChatTitle(chat.title);
      renderChatList(chats, activeChatId);
    });
    li.appendChild(btn);
    chatListEl.appendChild(li);
  }
}

function appendMessage(role, content) {
  const bubble = document.createElement("article");
  bubble.className =
    role === "user" ? "message message-user" : "message message-assistant";

  const meta = document.createElement("span");
  meta.className = "message-meta";
  meta.textContent = role === "user" ? "You" : "Assistant";

  const body = document.createElement("div");
  body.textContent = content;

  bubble.appendChild(meta);
  bubble.appendChild(body);
  messageListEl.appendChild(bubble);
  messageListEl.scrollTop = messageListEl.scrollHeight;
}

startForm.addEventListener("submit", (event) => {
  event.preventDefault();
  const text = startInput.value.trim();
  if (!text) {
    return;
  }

  // Step 8: POST /api/chats then POST /api/messages
  showAppView();
  setChatTitle(text.length > 48 ? `${text.slice(0, 48)}…` : text);
  clearMessages();
  appendMessage("user", text);
  renderChatList([], null);
  composerInput.value = "";
});

newChatBtn.addEventListener("click", () => {
  // Step 8: POST /api/chats and reset the thread
  activeChatId = null;
  setChatTitle("New chat");
  clearMessages();
  showAppView();
  composerInput.focus();
});

composerForm.addEventListener("submit", (event) => {
  event.preventDefault();
  const text = composerInput.value.trim();
  if (!text) {
    return;
  }

  // Step 8: POST /api/messages
  appendMessage("user", text);
  composerInput.value = "";
});

renderEmptyChatList();
showStartView();
