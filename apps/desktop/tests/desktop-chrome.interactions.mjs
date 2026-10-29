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
    assert(bounds.top === 0 && bounds.height <= Math.max(nativeHeight, 32) + 1 && !document.querySelector("#app > .titlebar-drag"), "Workspace commands occupy a second row below the native titlebar.");
    assert(toggle.getBoundingClientRect().left >= controlsWidth + 12 && getComputedStyle(document.querySelector("#app")).paddingTop === "0px", "Toolbar commands overlap native traffic lights or retain duplicate titlebar spacing.");
    assert(toggle.querySelector("svg") && !toggle.textContent.trim() && toggle.getAttribute("aria-label") && toggle.title, "Sidebar control must use a named standard glyph.");
    assert(search.querySelector("svg") && !search.textContent.trim() && search.getAttribute("aria-label") && search.title, "Search control must use a named magnifier glyph.");
    assert(!recording.classList.contains("primary") && getComputedStyle(recording).backgroundColor === "rgba(0, 0, 0, 0)" && recording.getBoundingClientRect().right <= search.getBoundingClientRect().left, "The recording action did not become a quiet trailing toolbar control.");
    toggle.focus();
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
    assert(getComputedStyle(toggle).fontFamily.includes("-apple-system") && getComputedStyle(document.querySelector(".library-shortcuts button")).fontFamily.includes("-apple-system"), "macOS chrome did not use system typography.");
    if (!initialVisible) toggle.click();
    const separator = document.querySelector(".workspace-separator");
    separator.focus();
    separator.dispatchEvent(new KeyboardEvent("keydown", { key: "End", bubbles: true, cancelable: true }));
    assert(separator.getAttribute("aria-valuenow") === "320", "Sidebar keyboard resizing did not reach its maximum.");
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
    assert(categories.length > 5, "Settings navigation unexpectedly lost categories.");
    await until(() => window.__PENNEL_DESKTOP_COMMAND_STATE__?.find === false, "Unavailable Settings search remained enabled in the native menu.");
    modal = document.createElement("dialog");
    const button = document.createElement("button"); button.textContent = "Owned modal action"; modal.append(button); document.body.append(modal); modal.showModal();
    await until(() => window.__PENNEL_DESKTOP_COMMAND_STATE__?.ready === false, "Native commands did not disable for the modal dialog.");
    fixture.emit("desktop-command", "meetings"); await settle();
    assert(document.querySelector(".settings-categories") && modal.open, "A native command bypassed the modal guard.");
    modal.close(); modal.remove(); modal = null;
    fixture.emit("desktop-command", "meetings");
    await until(() => document.querySelector(".meeting-detail") && window.__PENNEL_DESKTOP_COMMAND_STATE__?.ready, "The workspace did not reopen after closing its modal.");
    assert(document.querySelectorAll('.workspace-toolbar button[tabindex="0"]').length === 1, "Reenabling a toolbar control created multiple Tab stops.");
    return { checks: 26, native_appearance_verified: false };
  } finally {
    modal?.close(); modal?.remove();
    fixture.emit("desktop-environment-changed", original);
    if (stored === null) localStorage.removeItem("pennel.workspace.presentation.v1"); else localStorage.setItem("pennel.workspace.presentation.v1", stored);
  }
}
