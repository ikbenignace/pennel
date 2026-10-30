/** Run in an owned fixture page: import('/tests/history-navigation.interactions.mjs').then(module => module.checkHistoryNavigation()). */
export async function checkHistoryNavigation() {
  if (!/^(127\.0\.0\.1|localhost)$/.test(location.hostname) || !window.__PENNEL_FIXTURE__ || !window.__PENNEL_PLATFORM__) {
    throw new Error("History interaction checks require the owned synthetic fixture server.");
  }
  const client = window.__PENNEL_PLATFORM__;
  const original = client.invoke.bind(client);
  const operations = [];
  let entries = [
    { id: "history-nav-first", created_at_ms: 1, app_bundle_id: null, raw: "Synthetic text", text: "Synthetic text to keep selected.", status: "done", error: null, has_audio: false },
    { id: "history-nav-second", created_at_ms: 2, app_bundle_id: null, raw: "Synthetic text", text: "Unsaved synthetic text.", status: "unsaved", error: null, has_audio: false },
    { id: "history-nav-third", created_at_ms: 3, app_bundle_id: null, raw: "", text: "", status: "failed", error: "Synthetic recognition failure", has_audio: true },
  ];
  client.invoke = async (command, args = {}) => {
    operations.push({ command, args });
    if (command === "history") return structuredClone(entries);
    if (command === "copy_text") return null;
    if (command === "delete_history") { entries = entries.filter(entry => entry.id !== args.id); return null; }
    return original(command, args);
  };
  const { historyView } = await import("/src/views/history.ts");
  const { t } = await import("/src/i18n.ts");
  const target = document.getElementById("content");
  const previous = [...target.childNodes];
  const cleanup = [];
  const errors = [];
  const ctx = {
    info: window.__PENNEL_FIXTURE__.info(), visible: true, rerender() {},
    onDispose: work => cleanup.push(work), report: error => errors.push(String(error)),
  };
  const settle = () => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
  const assert = (value, message) => { if (!value) throw new Error(message); };
  const key = (node, name, options = {}) => {
    const event = new KeyboardEvent("keydown", { bubbles: true, cancelable: true, key: name, ...options });
    node.dispatchEvent(event);
    return event;
  };
  const results = [];
  try {
    target.replaceChildren(await historyView(ctx));
    await settle();
    const rows = [...target.querySelectorAll("[data-history-id]")];
    rows[0].focus();
    assert(rows.filter(row => row.tabIndex === 0).length === 1, "History must have one row entry in the tab order.");
    key(rows[0], "ArrowDown");
    assert(document.activeElement === rows[1] && rows[1].getAttribute("aria-current") === "true", "ArrowDown did not move focus and selection together.");
    key(rows[1], "End");
    assert(document.activeElement === rows[2], "End did not reach the last entry.");
    key(rows[2], "Home");
    assert(document.activeElement === rows[0], "Home did not reach the first entry.");
    const copy = rows[0].querySelector('[data-history-action="copy"]');
    copy.focus();
    assert(!key(copy, "ArrowDown").defaultPrevented && document.activeElement === copy, "Row navigation intercepted a child control.");
    rows[0].focus();
    assert(!key(rows[0], "ArrowDown", { shiftKey: true }).defaultPrevented, "Row navigation intercepted text-selection modifiers.");
    results.push("Arrow/Home/End navigation has one row tab stop and preserves child-control keyboard behavior.");

    const content = rows[0].querySelector(".history-content p");
    const selection = window.getSelection();
    const range = document.createRange();
    range.selectNodeContents(content);
    selection.removeAllRanges(); selection.addRange(range);
    const textMenu = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    content.dispatchEvent(textMenu);
    assert(!textMenu.defaultPrevented && selection.toString() === entries[0].text, "History replaced the selected-text context menu or lost the selection.");
    selection.removeAllRanges();
    results.push("Selected text retains the webview's standard context menu and selection.");

    rows[1].focus();
    key(rows[1], "F10", { shiftKey: true });
    await settle();
    const popup = document.querySelector('[role="menu"].context-menu');
    assert(popup, "History keyboard context menu did not open.");
    const menuButton = label => [...popup.querySelectorAll("button")].find(button => button.textContent.trim() === label);
    assert(menuButton(t("common.copy")) && menuButton(t("dictation.history.save_again")), "Unsaved entry actions are missing.");
    assert(!menuButton(t("common.retry")), "Unsaved entry unexpectedly offers recognition retry.");
    menuButton(t("common.delete")).click();
    await settle();
    const dialog = document.querySelector("dialog[open]");
    assert(dialog, "Context-menu discard bypassed the existing confirmation.");
    [...dialog.querySelectorAll("button")].find(button => button.textContent.trim() === t("common.cancel")).click();
    await settle();
    assert(entries.length === 3 && !operations.some(operation => operation.command === "delete_history"), "Cancelling discarded an entry.");
    assert(document.activeElement === rows[1], "Dismissal did not restore row focus.");
    assert(errors.length === 0, errors.join("\n"));
    results.push("Context actions reuse existing unsaved-data confirmation and restore focus after cancellation.");
    return { passed: results.length, results, userDataUsed: false, externalServicesUsed: false };
  } finally {
    cleanup.forEach(work => work());
    client.invoke = original;
    target.replaceChildren(...previous);
  }
}
