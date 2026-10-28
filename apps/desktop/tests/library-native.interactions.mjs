const assert = (value, message) => { if (!value) throw new Error(message); };
const pause = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds));
async function until(check, message) {
  const deadline = performance.now() + 6000;
  while (!check()) {
    if (performance.now() > deadline) throw new Error(message);
    await pause(20);
  }
}

export async function checkStableLibrary() {
  const fixture = window.__PENNEL_FIXTURE__;
  const client = window.__PENNEL_PLATFORM__;
  assert(fixture && client?.fixture, "Use an owned synthetic fixture page.");
  await until(() => document.querySelectorAll(".meeting-row").length > 1, "The library did not load.");
  const before = [...document.querySelectorAll(".meeting-row")];
  const selected = document.querySelector(".meeting-select[aria-current]");
  const first = before[0].querySelector(".meeting-select");
  first.focus();
  const originalInvoke = client.invoke;
  const calls = [];
  let block = null;
  let protectedId = null;
  let omittedId = null;
  let reversed = false;
  client.invoke = async function(command, args) {
    calls.push(command);
    if (command === "meetings" && block) { const wait = block; block = null; await wait; }
    const result = await originalInvoke.call(this, command, args);
    if (command === "meetings" && omittedId) return result.filter((meeting) => meeting.id !== omittedId);
    if (command === "meetings" && reversed) return [...result].reverse();
    return command === "meeting_runtime_status" && protectedId
      ? { ...result, busy: true, recording: true, phase: "recording", id: protectedId, session_id: "owned-menu-guard" }
      : result;
  };
  const meetingReads = () => calls.filter((command) => command === "meetings" || command === "search_library").length;
  let release;
  try {
    block = new Promise((resolve) => { release = resolve; });
    fixture.emit("library-changed", null);
    await until(() => meetingReads() === 1, "Library invalidation did not request a refresh.");
    fixture.emit("library-changed", null);
    fixture.emit("library-changed", null);
    release();
    await until(() => meetingReads() >= 2, "An invalidation during refresh was lost.");
    await pause(100);
    assert(meetingReads() === 2, "Overlapping library invalidations were not coalesced.");
    assert(before.every((row, index) => document.querySelectorAll(".meeting-row")[index] === row), "Unchanged rows were replaced during refresh.");
    assert(document.activeElement === first, "Refreshing the library stole keyboard focus.");
    assert(document.querySelector(".meeting-select[aria-current]") === selected, "Refreshing the library changed the selected meeting.");
    assert(!calls.includes("meeting_status"), "The sidebar fetched transcript content during a metadata refresh.");
    const list = document.querySelector(".meeting-list");
    const scroll = list.scrollTop;
    const reads = meetingReads();
    fixture.emit("library-changed", null);
    await until(() => meetingReads() > reads, "The repeated refresh did not complete.");
    await pause(50);
    assert(list.scrollTop === scroll, "A library refresh reset its scroll position.");
    first.dispatchEvent(new KeyboardEvent("keydown", { key: "F10", shiftKey: true, bubbles: true }));
    await until(() => document.querySelector(".context-menu"), "The keyboard context menu did not open.");
    assert(document.querySelector(".context-menu").contains(document.activeElement), "The context menu did not receive keyboard focus.");
    document.activeElement.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    await until(() => !document.querySelector(".context-menu"), "Escape did not dismiss the context menu.");

    protectedId = before[0].dataset.id;
    const guardReads = meetingReads();
    fixture.emit("library-changed", null);
    await until(() => meetingReads() > guardReads, "The guarded status did not refresh.");
    await pause(50);
    before[0].querySelector(".meeting-more").click();
    await until(() => document.querySelector(".context-menu"), "The guarded context menu did not open.");
    const items = [...document.querySelectorAll(".context-menu button")];
    assert(items.length === 3 && items[0].disabled && items[2].disabled, "Active recording mutation actions were enabled.");
    assert(!items[1].disabled, "The harmless folder action was unnecessarily disabled.");
    document.activeElement.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    protectedId = null;
    fixture.emit("library-changed", null);

    document.querySelector(".library-settings").click();
    await until(() => document.querySelector(".settings-workspace"), "Settings did not open.");
    await pause(100);
    const hiddenReads = meetingReads();
    fixture.emit("library-changed", null);
    fixture.emit("library-changed", null);
    await pause(150);
    assert(meetingReads() === hiddenReads, "The hidden library still fetched its rows.");
    document.querySelector(".settings-back").click();
    await until(() => !document.querySelector(".settings-workspace"), "Settings did not return to the meeting.");
    await until(() => meetingReads() > hiddenReads, "Returning to the library lost a deferred invalidation.");

    first.focus();
    first.dispatchEvent(new KeyboardEvent("keydown", { key: "End", bubbles: true }));
    const last = before.at(-1).querySelector(".meeting-select");
    assert(document.activeElement === last, "End did not focus the last meeting.");
    last.dispatchEvent(new KeyboardEvent("keydown", { key: "Home", bubbles: true }));
    assert(document.activeElement === first, "Home did not focus the first meeting.");
    const lastTitle = before.at(-1).querySelector(".meeting-title").textContent;
    first.dispatchEvent(new KeyboardEvent("keydown", { key: lastTitle[0], bubbles: true }));
    assert(document.activeElement === last, "Type-to-select did not focus the matching meeting.");
    omittedId = before.at(-1).dataset.id;
    fixture.emit("library-changed", null);
    await until(() => !document.querySelector(`[data-id="${CSS.escape(omittedId)}"]`), "The removed row did not leave the library.");
    assert(document.activeElement === before.at(-2).querySelector(".meeting-select"), "Removing a focused row lost keyboard focus instead of selecting a neighbor.");
    omittedId = null;
    fixture.emit("library-changed", null);
    await until(() => document.querySelectorAll(".meeting-row").length === before.length, "The synthetic library did not restore its rows.");
    const restoredLast = document.querySelector(`[data-id="${CSS.escape(before.at(-1).dataset.id)}"] .meeting-select`);
    restoredLast.focus();
    const titleNode = restoredLast.querySelector(".meeting-title").firstChild;
    document.getSelection().setBaseAndExtent(titleNode, 0, titleNode, 3);
    assert(document.getSelection().anchorNode === titleNode && document.getSelection().focusNode === titleNode, "The synthetic range probe did not target its source row.");
    reversed = true;
    fixture.emit("library-changed", null);
    await until(() => document.querySelector(".meeting-row").dataset.id === before.at(-1).dataset.id, "The reordered library did not render.");
    assert(document.activeElement === restoredLast, "Moving a source row lost keyboard focus.");
    const range = document.getSelection();
    assert(range.anchorNode === titleNode && range.anchorOffset === 0 && range.focusNode === titleNode && range.focusOffset === 3, "Moving a source row lost its selection boundaries.");
    reversed = false;
    fixture.emit("library-changed", null);
    assert(!calls.some((command) => ["start_meeting", "stop_meeting"].includes(command)), "Library navigation changed recording state.");
    assert(!fixture.unknown.length, "The library made an unknown fixture call.");
    return { stableRows: true, focusPreserved: true, scrollPreserved: true, coalescedRefresh: true, hiddenRefreshDeferred: true, keyboardSourceList: true, keyboardMenus: true, activeRecordingGuards: true, metadataOnly: true };
  } finally {
    release?.();
    protectedId = null;
    omittedId = null;
    reversed = false;
    client.invoke = originalInvoke;
  }
}
