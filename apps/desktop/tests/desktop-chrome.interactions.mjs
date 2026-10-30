const assert = (value, message) => { if (!value) throw new Error(message); };
const settle = () => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
async function until(check, message) {
  for (let attempt = 0; attempt < 150; attempt++) {
    if (check()) return;
    await new Promise(resolve => setTimeout(resolve, 30));
  }
  throw new Error(message);
}

/** Uses synthetic events on the owned fixture only; native material appearance needs separate app captures. */
export async function checkDesktopChrome() {
  const fixture = window.__PENNEL_FIXTURE__;
  assert(location.hostname === "127.0.0.1" && fixture && window.__PENNEL_PLATFORM__, "Chrome checks require the owned fixture server.");
  const { api } = await import("/src/api.ts");
  await until(() => document.querySelector(".own-notes textarea") && window.__PENNEL_DESKTOP_COMMAND_STATE__?.ready, "The workspace did not become ready.");
  const original = await api.desktopEnvironment();
  const root = document.documentElement;
  const note = document.querySelector(".own-notes textarea");
  const container = document.querySelector(".meeting-detail");
  const stored = localStorage.getItem("pennel.workspace.presentation.v1");
  const toggle = document.querySelector('[data-command="toggle_sidebar"]');
  const initialVisible = root.dataset.sidebarVisible !== "false";
  let modal;
  try {
    assert(document.querySelectorAll(".library-record").length === 1 && document.querySelector(".workspace-toolbar .library-record"), "The recording action was duplicated instead of retained in the toolbar.");
    const toolbar = document.querySelector(".workspace-toolbar");
    const search = toolbar.querySelector('[data-command="find"]');
    const recording = toolbar.querySelector(".toolbar-recording");
    const bounds = toolbar.getBoundingClientRect();
    const nativeHeight = parseFloat(getComputedStyle(root).getPropertyValue("--native-titlebar-height"));
    const controlsWidth = parseFloat(getComputedStyle(root).getPropertyValue("--native-titlebar-controls-width"));
    assert(bounds.top === 0 && Math.abs(bounds.height - Math.max(nativeHeight, 52)) < 1 && !document.querySelector("#app > .titlebar-drag"), "Workspace commands must share the padded 52 pt native titlebar row.");
    assert(toggle.getBoundingClientRect().left >= controlsWidth + 12 && getComputedStyle(document.querySelector("#app")).paddingTop === "0px", "Toolbar commands overlap native traffic lights or retain duplicate titlebar spacing.");
    assert(toggle.querySelector("svg") && !toggle.textContent.trim() && toggle.getAttribute("aria-label") && toggle.title, "Sidebar control must use a named standard glyph.");
    assert(search.querySelector("svg") && !search.textContent.trim() && search.getAttribute("aria-label") && search.title, "Search control must use a named magnifier glyph.");
    for (const control of [toggle, search]) {
      const rect = control.getBoundingClientRect();
      const style = getComputedStyle(control);
      assert(rect.width === 36 && rect.height === 36 && parseFloat(style.borderRadius) >= 18 && rect.top >= bounds.top + 8 && rect.bottom <= bounds.bottom - 8, "Toolbar icon targets must be 36 pt circles with clear vertical breathing room.");
      assert(control.querySelector("svg").getBoundingClientRect().width === 18 && style.backgroundColor !== "rgba(0, 0, 0, 0)" && style.boxShadow !== "none", "Toolbar controls lost their readable raised boundary or 18 pt glyph.");
    }
    const recordRect = recording.getBoundingClientRect();
    const recordStyle = getComputedStyle(recording);
    assert(!recording.classList.contains("primary") && recordRect.height === 36 && parseFloat(recordStyle.borderRadius) >= 18 && parseFloat(recordStyle.paddingLeft) >= 12 && parseFloat(recordStyle.paddingRight) >= 12 && recordRect.right + 12 <= search.getBoundingClientRect().left, "Recording must remain a separate padded 36 pt capsule with a clear action gap.");
    toggle.focus();
    assert(getComputedStyle(toggle).outlineStyle !== "none" && parseFloat(getComputedStyle(toggle).outlineWidth) >= 2, "Keyboard focus is not visibly distinct from the control boundary.");
    toggle.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true, cancelable: true }));
    assert(document.activeElement?.matches(".toolbar-recording"), "Toolbar arrow navigation did not reach the recording action.");
    note.closest("details").open = true;
    note.focus(); note.setSelectionRange(2, 9);
    const value = note.value;
    const glass = { ...original, material: "liquid_glass", reduce_transparency: false, increase_contrast: false, focused: true };
    for (const environment of [glass, { ...glass, reduce_motion: true }, { ...glass, reduce_transparency: true }, { ...glass, increase_contrast: true }, { ...glass, focused: false }, glass]) {
      fixture.emit("desktop-environment-changed", environment);
      await settle();
      assert(document.querySelector(".meeting-detail") === container && document.querySelector(".own-notes textarea") === note, "An OS environment change remounted the document.");
      assert(document.activeElement === note && note.value === value && note.selectionStart === 2 && note.selectionEnd === 9, "An OS environment change lost editor focus, draft or selection.");
      assert(root.dataset.nativeMaterial === (environment.reduce_transparency || environment.increase_contrast ? "opaque" : "liquid_glass"), "Accessibility preferences did not choose an opaque material.");
    }
    assert(getComputedStyle(document.querySelector(".workspace-body")).backgroundColor !== "rgba(0, 0, 0, 0)", "The reading surface became transparent.");
    const navigationControl = document.querySelector(".library-shortcuts button");
    const usesFigtree = control => getComputedStyle(control).fontFamily.split(",")[0].replaceAll('"', "").trim() === "Figtree";
    assert(usesFigtree(toggle) && usesFigtree(navigationControl), "macOS chrome did not retain the shared Figtree typography.");
    assert(parseFloat(getComputedStyle(navigationControl).fontSize) >= 13 && navigationControl.getBoundingClientRect().height >= 32, "Source navigation lost its readable labels or 32 pt targets.");
    if (!initialVisible) toggle.click();
    const separator = document.querySelector(".workspace-separator");
    separator.focus();
    separator.dispatchEvent(new KeyboardEvent("keydown", { key: "End", bubbles: true, cancelable: true }));
    assert(separator.getAttribute("aria-valuenow") === "320", "Sidebar keyboard resizing did not reach its maximum.");
    await settle();
    assert(document.documentElement.scrollWidth <= innerWidth, "Maximum sidebar width created horizontal overflow.");
    assert(Math.abs(document.querySelector(".toolbar-section").getBoundingClientRect().left - document.querySelector(".meeting-header h1").getBoundingClientRect().left) < 1, "Toolbar context and the reading column lost their shared left alignment.");
    const footer = document.querySelector(".meeting-footer").getBoundingClientRect();
    for (const control of document.querySelectorAll(".meeting-footer .btn,.meeting-footer .menu-button")) {
      if (!control.getClientRects().length) continue;
      const rect = control.getBoundingClientRect();
      assert(rect.height >= 28 && rect.left >= footer.left && rect.right <= footer.right && rect.bottom <= footer.bottom, `Footer actions became clipped or crowded at maximum sidebar width: ${JSON.stringify({ control: control.className, rect: rect.toJSON(), footer: footer.toJSON() })}`);
    }
    separator.dispatchEvent(new KeyboardEvent("keydown", { key: "Home", bubbles: true, cancelable: true }));
    separator.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true, cancelable: true }));
    assert(separator.getAttribute("aria-valuenow") === "204", "Sidebar keyboard resizing did not use the shared step.");
    separator.focus();
    fixture.emit("desktop-command", "toggle_sidebar");
    assert(root.dataset.sidebarVisible === "false" && getComputedStyle(document.querySelector(".library-sidebar")).display === "none", "The sidebar did not collapse.");
    assert(document.activeElement === toggle, "Collapsing the sidebar stranded focus on its hidden resize separator.");
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "f", metaKey: true, bubbles: true, cancelable: true }));
    assert(root.dataset.sidebarVisible === "true" && document.activeElement?.matches(".library-search input"), "Find did not restore and focus the collapsed sidebar.");
    assert(document.querySelector(".meeting-detail") === container, "Sidebar actions replaced the open meeting.");

    toggle.focus();
    toggle.dispatchEvent(new KeyboardEvent("keydown", { key: "End", bubbles: true, cancelable: true }));
    fixture.emit("desktop-command", "settings");
    await until(() => document.querySelector(".settings-categories") && !document.querySelector("#content").inert, "The native Settings command did not use workspace navigation.");
    const categories = document.querySelectorAll("[data-settings-category]");
    const selected = document.querySelector('[data-settings-category][aria-current="page"]');
    selected.focus();
    selected.dispatchEvent(new KeyboardEvent("keydown", { key: "End", bubbles: true, cancelable: true }));
    await until(() => document.querySelector('[data-settings-category="models"][aria-current="page"]') && !document.querySelector("#content").inert, "End did not select the last Settings category.");
    assert(document.activeElement?.dataset.settingsCategory === "models", "Settings category navigation lost focus after mounting.");
    assert(Math.abs(document.querySelector(".toolbar-section").getBoundingClientRect().left - document.querySelector(".settings-title").getBoundingClientRect().left) < 1, "Settings context and content lost their shared left inset.");
    assert(categories.length > 5, "Settings navigation unexpectedly lost categories.");
    await until(() => window.__PENNEL_DESKTOP_COMMAND_STATE__?.find === false, "Unavailable Settings search remained enabled in the native menu.");
    const disabledSearch = getComputedStyle(search);
    assert(search.disabled && disabledSearch.opacity === "1" && disabledSearch.color !== "rgba(0, 0, 0, 0)" && disabledSearch.backgroundColor !== "rgba(0, 0, 0, 0)", "Disabled toolbar controls lost their visible label or surface.");
    modal = document.createElement("dialog");
    const button = document.createElement("button"); button.textContent = "Owned modal action"; modal.append(button); document.body.append(modal); modal.showModal();
    await until(() => window.__PENNEL_DESKTOP_COMMAND_STATE__?.ready === false, "Native commands did not disable for the modal dialog.");
    fixture.emit("desktop-command", "meetings"); await settle();
    assert(document.querySelector(".settings-categories") && modal.open, "A native command bypassed the modal guard.");
    modal.close(); modal.remove(); modal = null;
    fixture.emit("desktop-command", "meetings");
    await until(() => document.querySelector(".meeting-detail") && window.__PENNEL_DESKTOP_COMMAND_STATE__?.ready, "The workspace did not reopen after closing its modal.");
    assert(document.querySelectorAll('.workspace-toolbar button[tabindex="0"]').length === 1, "Reenabling a toolbar control created multiple Tab stops.");
    return { checks: 33, native_appearance_verified: false };
  } finally {
    modal?.close(); modal?.remove();
    fixture.emit("desktop-environment-changed", original);
    if (stored === null) localStorage.removeItem("pennel.workspace.presentation.v1"); else localStorage.setItem("pennel.workspace.presentation.v1", stored);
  }
}
