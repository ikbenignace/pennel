/** Browser verification against owned synthetic fixtures only; no real audio is used here. */
export async function checkRetranscription() {
  if (!/^(localhost|127\.0\.0\.1)$/.test(location.hostname) || !window.__PENNEL_FIXTURE__?.options.saved_audio) throw new Error("Owned saved-audio fixture required");
  const { t } = await import("../src/i18n.ts");
  const client = window.__PENNEL_PLATFORM__;
  const original = client.invoke.bind(client);
  const commands = [];
  let failPreflight = false;
  client.invoke = (command, args) => {
    commands.push(command);
    if (failPreflight && command === "meeting_detail") { failPreflight = false; return Promise.reject(new Error("Synthetic preflight failure")); }
    return original(command, args);
  };
  const assert = (condition, message) => { if (!condition) throw new Error(message); };
  const findButton = (scope, label) => [...scope.querySelectorAll("button")].find(button => button.textContent.trim() === label);
  const until = async (check, message) => {
    for (let attempt = 0; attempt < 120; attempt++) {
      if (check()) return;
      await new Promise(resolve => setTimeout(resolve, 40));
    }
    throw new Error(message);
  };
  const action = () => findButton(document, t("meetings.retranscribe_audio"));
  try {
    await until(() => action() && !action().disabled, "Local retranscription is unavailable");
    document.querySelector('[data-tab="notes"]').click();
    const notes = document.querySelector(".own-notes textarea");
    const expectedNotes = notes.value + "\nSynthetische eigen notitie blijft bewaard.";
    notes.value = expectedNotes;
    notes.dispatchEvent(new Event("input", { bubbles: true }));
    document.querySelector('[data-tab="transcript"]').click();
    action().click();
    await until(() => document.querySelector("dialog[open]"), "Confirmation did not open");
    let confirmation = document.querySelector("dialog[open]");
    assert(confirmation.textContent.includes(window.__PENNEL_FIXTURE__.info().settings.meeting_stt.model), "Confirmation does not show the chosen local model");
    findButton(confirmation, t("common.cancel")).click();
    await until(() => !document.querySelector("dialog[open]"), "Cancellation did not close confirmation");
    assert(!commands.includes("retranscribe_meeting_audio"), "Cancelling started transcription");
    failPreflight = true;
    action().click();
    await until(() => document.querySelector(".meeting-message")?.textContent.includes("Synthetic preflight failure"), "A failed preflight has no visible recovery message");
    assert(!action().disabled, "A failed preflight left the retry action disabled");
    action().click();
    await until(() => document.querySelector("dialog[open]"), "Confirmation did not reopen");
    confirmation = document.querySelector("dialog[open]");
    findButton(confirmation, t("meetings.retranscribe_audio")).click();
    await until(() => action()?.disabled && document.querySelector(".meeting-message")?.textContent.includes("25%"), "Progress or duplicate-action protection is missing");
    assert(!commands.includes("summarize_meeting"), "Retranscription started a summary request");
    await until(() => action() && !action().disabled && document.body.innerText.includes(t("meetings.summary_outdated")), "The completed replacement did not mark the old summary outdated");
    document.querySelector('[data-tab="notes"]').click();
    assert(document.querySelector(".own-notes textarea").value === expectedNotes, "Retranscription lost the user's note draft");
    document.querySelector('[data-tab="transcript"]').click();
    return { confirmed: true, progress: true, notesPreserved: true, summaryOutdated: true };
  } finally {
    client.invoke = original;
  }
}
