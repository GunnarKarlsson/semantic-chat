/**
 * Semantic Chat UI — talks to the local Axum API.
 * UI chats (/api/chats, /api/messages) are separate from remote Chat Completions.
 */

const MESSAGE_TIMEOUT_MS = 60_000;
/** Extra space above a newly arrived reply when aligning it into view. */
const REPLY_TOP_GAP_PX = 50;

const viewStart = document.getElementById("view-start");
const viewApp = document.getElementById("view-app");
const startForm = document.getElementById("start-form");
const startInput = document.getElementById("start-input");
const newChatBtn = document.getElementById("new-chat-btn");
const settingsBtn = document.getElementById("settings-btn");
const settingsDialog = document.getElementById("settings-dialog");
const settingsCloseBtn = document.getElementById("settings-close-btn");
const clearCacheBtn = document.getElementById("clear-cache-btn");
const cacheEntryCountEl = document.getElementById("cache-entry-count");
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
  settingsBtn.disabled = isBusy;
  clearCacheBtn.disabled = isBusy;
  const submitButtons = document.querySelectorAll(
    '#start-form button[type="submit"], #composer-form button[type="submit"]'
  );
  for (const button of submitButtons) {
    button.disabled = isBusy;
  }
}

function setCacheEntryCount(count) {
  cacheEntryCountEl.textContent = String(count ?? 0);
}

async function loadSettings() {
  const settings = await api("/api/settings");
  setCacheEntryCount(settings.cache_entry_count);
  return settings;
}

async function openSettings() {
  if (busy) {
    return;
  }

  setBusy(true);
  try {
    await loadSettings();
    if (typeof settingsDialog.showModal === "function") {
      settingsDialog.showModal();
    } else {
      settingsDialog.setAttribute("open", "");
    }
  } catch (error) {
    console.error(error);
    window.alert(`Could not load settings: ${error.message}`);
  } finally {
    setBusy(false);
  }
}

function closeSettings() {
  if (typeof settingsDialog.close === "function") {
    settingsDialog.close();
  } else {
    settingsDialog.removeAttribute("open");
  }
}

async function clearCache() {
  if (busy) {
    return;
  }

  const confirmed = window.confirm(
    "Clear all cached answers? Your chats will stay."
  );
  if (!confirmed) {
    return;
  }

  setBusy(true);
  try {
    const settings = await api("/api/settings/clear-cache", { method: "POST" });
    setCacheEntryCount(settings.cache_entry_count);
  } catch (error) {
    console.error(error);
    window.alert(`Could not clear cache: ${error.message}`);
  } finally {
    setBusy(false);
  }
}

function setChatTitle(title) {
  chatTitleEl.textContent = title || "New chat";
}

function clearMessages() {
  messageListEl.innerHTML = "";
}

function scrollMessageListToEnd() {
  messageListEl.scrollTop = messageListEl.scrollHeight;
}

/** Pin a bubble's top near the top of the visible message area. */
function alignBubbleToTop(bubble) {
  if (!bubble) {
    return;
  }
  const listRect = messageListEl.getBoundingClientRect();
  const bubbleRect = bubble.getBoundingClientRect();
  // Use viewport delta so this stays correct for every reply, not only the first.
  messageListEl.scrollTop += bubbleRect.top - listRect.top - REPLY_TOP_GAP_PX;
}

function scheduleAlignBubbleToTop(bubble) {
  // Wait for layout after inserting a (possibly tall) reply.
  requestAnimationFrame(() => {
    requestAnimationFrame(() => alignBubbleToTop(bubble));
  });
}

function escapeHtml(text) {
  return String(text)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** Inline markdown on already-escaped text. */
function renderInlineMarkdown(escaped) {
  return escaped
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/__([^_]+)__/g, "<strong>$1</strong>")
    .replace(/(^|[^*\w])\*([^*\n]+)\*(?!\*)/g, "$1<em>$2</em>")
    .replace(/(^|[^_\w])_([^_\n]+)_(?!_)/g, "$1<em>$2</em>");
}

function splitTableCells(line) {
  let row = line.trim();
  if (row.startsWith("|")) {
    row = row.slice(1);
  }
  if (row.endsWith("|")) {
    row = row.slice(0, -1);
  }
  return row.split("|").map((cell) => cell.trim());
}

function isTableSeparator(line) {
  const cells = splitTableCells(line);
  if (!cells.length) {
    return false;
  }
  return cells.every((cell) => /^:?-{3,}:?$/.test(cell.replace(/\s/g, "")));
}

function looksLikeTableRow(line) {
  const trimmed = line.trim();
  return trimmed.includes("|") && (trimmed.startsWith("|") || /\|/.test(trimmed));
}

function parseAlignments(separatorLine) {
  return splitTableCells(separatorLine).map((cell) => {
    const compact = cell.replace(/\s/g, "");
    const left = compact.startsWith(":");
    const right = compact.endsWith(":");
    if (left && right) {
      return "center";
    }
    if (right) {
      return "right";
    }
    return "left";
  });
}

function renderMarkdownTable(headerLine, separatorLine, bodyLines) {
  const headers = splitTableCells(headerLine);
  const alignments = parseAlignments(separatorLine);
  const colCount = headers.length;

  const ths = headers
    .map((cell, index) => {
      const align = alignments[index] || "left";
      return `<th style="text-align:${align}">${renderInlineMarkdown(cell)}</th>`;
    })
    .join("");

  const rows = bodyLines
    .map((line) => {
      const cells = splitTableCells(line);
      while (cells.length < colCount) {
        cells.push("");
      }
      const tds = cells
        .slice(0, colCount)
        .map((cell, index) => {
          const align = alignments[index] || "left";
          return `<td style="text-align:${align}">${renderInlineMarkdown(cell)}</td>`;
        })
        .join("");
      return `<tr>${tds}</tr>`;
    })
    .join("");

  return `<div class="table-wrap"><table><thead><tr>${ths}</tr></thead><tbody>${rows}</tbody></table></div>`;
}

/**
 * Safe subset of Markdown → HTML (escape first, then add our own tags).
 * Supports: headings, **bold**, *italic*, `code`, fences, lists, tables, paragraphs.
 */
function markdownToHtml(src) {
  const text = String(src ?? "");
  const fences = [];
  const withPlaceholders = text.replace(/```([\s\S]*?)```/g, (_, code) => {
    const index = fences.length;
    fences.push(`<pre><code>${escapeHtml(code.replace(/^\n+|\n+$/g, ""))}</code></pre>`);
    return `\u0000FENCE${index}\u0000`;
  });

  const escaped = escapeHtml(withPlaceholders);
  const lines = escaped.split("\n");
  const blocks = [];
  let paragraph = [];
  let listItems = [];
  let listTag = null;
  let index = 0;

  function flushParagraph() {
    if (!paragraph.length) {
      return;
    }
    const html = renderInlineMarkdown(paragraph.join("<br>"));
    blocks.push(`<p>${html}</p>`);
    paragraph = [];
  }

  function flushList() {
    if (!listTag || !listItems.length) {
      listItems = [];
      listTag = null;
      return;
    }
    const items = listItems
      .map((item) => `<li>${renderInlineMarkdown(item)}</li>`)
      .join("");
    blocks.push(`<${listTag}>${items}</${listTag}>`);
    listItems = [];
    listTag = null;
  }

  while (index < lines.length) {
    const line = lines[index];

    const fenceMatch = line.match(/^\u0000FENCE(\d+)\u0000$/);
    if (fenceMatch) {
      flushParagraph();
      flushList();
      blocks.push(fences[Number(fenceMatch[1])]);
      index += 1;
      continue;
    }

    // GFM pipe table: header + separator + body rows
    if (
      looksLikeTableRow(line) &&
      index + 1 < lines.length &&
      isTableSeparator(lines[index + 1])
    ) {
      flushParagraph();
      flushList();
      const headerLine = line;
      const separatorLine = lines[index + 1];
      const bodyLines = [];
      index += 2;
      while (index < lines.length && looksLikeTableRow(lines[index])) {
        bodyLines.push(lines[index]);
        index += 1;
      }
      blocks.push(renderMarkdownTable(headerLine, separatorLine, bodyLines));
      continue;
    }

    const heading = line.match(/^(#{1,6})\s+(.+?)(?:\s+#*)?$/);
    if (heading) {
      flushParagraph();
      flushList();
      const level = heading[1].length;
      blocks.push(
        `<h${level}>${renderInlineMarkdown(heading[2].trim())}</h${level}>`
      );
      index += 1;
      continue;
    }

    const unordered = line.match(/^[-*]\s+(.+)$/);
    if (unordered) {
      flushParagraph();
      if (listTag && listTag !== "ul") {
        flushList();
      }
      listTag = "ul";
      listItems.push(unordered[1]);
      index += 1;
      continue;
    }

    const ordered = line.match(/^\d+\.\s+(.+)$/);
    if (ordered) {
      flushParagraph();
      if (listTag && listTag !== "ol") {
        flushList();
      }
      listTag = "ol";
      listItems.push(ordered[1]);
      index += 1;
      continue;
    }

    if (line.trim() === "") {
      flushParagraph();
      flushList();
      index += 1;
      continue;
    }

    flushList();
    paragraph.push(line);
    index += 1;
  }

  flushParagraph();
  flushList();
  return blocks.join("") || `<p>${renderInlineMarkdown(escaped)}</p>`;
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
  body.className = "message-body";
  if (options.error) {
    body.textContent = content;
  } else {
    body.innerHTML = markdownToHtml(content);
  }

  bubble.appendChild(meta);
  bubble.appendChild(body);
  messageListEl.appendChild(bubble);

  // `top` = start reading the reply; `end` = keep latest composer activity in view
  const scroll = options.scroll || "end";
  if (scroll === "top") {
    scheduleAlignBubbleToTop(bubble);
  } else if (scroll !== "none") {
    scrollMessageListToEnd();
  }
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
  scrollMessageListToEnd();
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
    li.className = "chat-list-row";
    if (chat.id === activeChatId) {
      li.classList.add("is-active");
    }

    const openBtn = document.createElement("button");
    openBtn.type = "button";
    openBtn.className = "chat-list-item";
    openBtn.textContent = chat.title || "New chat";
    openBtn.addEventListener("click", () => {
      void selectChat(chat.id);
    });

    const deleteBtn = document.createElement("button");
    deleteBtn.type = "button";
    deleteBtn.className = "chat-delete-btn";
    deleteBtn.title = "Delete chat";
    deleteBtn.setAttribute("aria-label", `Delete chat: ${chat.title || "New chat"}`);
    deleteBtn.textContent = "×";
    deleteBtn.addEventListener("click", (event) => {
      event.stopPropagation();
      void deleteChat(chat.id, chat.title || "New chat");
    });

    li.appendChild(openBtn);
    li.appendChild(deleteBtn);
    chatListEl.appendChild(li);
  }
}

async function deleteChat(chatId, title) {
  if (busy) {
    return;
  }

  const confirmed = window.confirm(
    `Delete chat “${title}”? This cannot be undone.`
  );
  if (!confirmed) {
    return;
  }

  setBusy(true);
  try {
    await api(`/api/chats/${chatId}`, { method: "DELETE" });
    chatSummaries = chatSummaries.filter((chat) => chat.id !== chatId);

    if (activeChatId === chatId) {
      activeChatId = null;
      setChatTitle("New chat");
      clearMessages();
      showStartView();
    }

    renderChatList();
  } catch (error) {
    console.error(error);
    window.alert(`Could not delete chat: ${error.message}`);
  } finally {
    setBusy(false);
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
    const replyBubble = appendMessage("assistant", result.reply, {
      cacheHit: result.cache_hit,
      scroll: "top",
    });
    setChatTitle(result.title);
    await refreshChatList();
    // Sidebar refresh can shift layout; re-pin the reply afterward.
    scheduleAlignBubbleToTop(replyBubble);
    return result;
  } catch (error) {
    removeTypingIndicator(typing);
    if (isAbortError(error)) {
      appendMessage("assistant", "timeout. try again.", {
        error: true,
        scroll: "top",
      });
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

settingsBtn.addEventListener("click", () => {
  void openSettings();
});

settingsCloseBtn.addEventListener("click", () => {
  closeSettings();
});

clearCacheBtn.addEventListener("click", () => {
  void clearCache();
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
