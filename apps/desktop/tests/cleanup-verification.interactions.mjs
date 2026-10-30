export async function checkCleanupVerification() {
  if (!/^(127\.0\.0\.1|localhost)$/.test(location.hostname) || !window.__PENNEL_FIXTURE__ || !window.__PENNEL_PLATFORM__) throw new Error("Use the owned synthetic fixture page.");
  const { dictationView } = await import("../src/views/dictation.ts");
  const { getInterfaceLanguage, t } = await import("../src/i18n.ts");
  const client = window.__PENNEL_PLATFORM__;
  const original = client.invoke.bind(client);
  const info = structuredClone(window.__PENNEL_FIXTURE__.info());
  const provider = { id: "cleanup-qa", kind: "custom", name: "Synthetic decision server", base_url: "https://decision.example/v1" };
  info.settings.providers.push(provider);
  info.settings.decision = { provider_id: provider.id, protocol: "mercury", model: "fixture-verifier", expected_model: null, upstream: null, max_input_bytes: 16000, timeout_secs: 15 };
  info.settings.mode = "clean";
  info.cleanup_verification = { enabled: false, available: false, reason_code: "unqualified", provider_id: provider.id, model: "fixture-verifier", endpoint: "https://decision.example/v1/decisions" };
  const tasks = JSON.stringify([info.settings.dictation_stt, info.settings.meeting_stt, info.settings.llm, info.settings.decision]);
  const calls = [];
  client.invoke = async (command, args) => {
    if (/inference|decide|rerank/.test(command)) calls.push(command);
    return original(command, args);
  };
  const target = document.getElementById("content");
  const previous = [...target.childNodes];
  let saveFailure = false;
  const errors = [];
  const context = {
    get info() { return info; },
    async save(change) {
      if (saveFailure) throw new Error("Synthetic cleanup preference save failure");
      change(info.settings); info.cleanup_verification.enabled = info.settings.cleanup_verification_enabled === true; mount();
    },
    rerender() { mount(); },
    report(error) { errors.push(String(error)); },
    navigate() {},
  };
  const mount = () => target.replaceChildren(dictationView(context));
  const panel = () => target.querySelector("[data-cleanup-verification]");
  const control = () => panel().querySelector('input[role="switch"]');
  const assert = (condition, message) => { if (!condition) throw new Error(message); };
  const results = [];
  try {
    for (const language of [getInterfaceLanguage()]) {
      info.settings.cleanup_verification_enabled = false;
      info.cleanup_verification.enabled = false;
      mount();
      assert(!control().checked && control().disabled, "An unqualified configured connection enabled verification.");
      assert(panel().textContent.includes(t("dictation.verification.saved_disabled")) && panel().textContent.includes(t("dictation.verification.unqualified")), "Saved enablement and qualification were not explained separately.");
      assert(panel().textContent.includes("decision.example") && !panel().textContent.includes(t("dictation.verification.active")), "The selected destination is missing or shown as active.");
      results.push(`${language}: configured but unqualified verification stays off with its destination explained.`);
      info.settings.cleanup_verification_enabled = true;
      info.cleanup_verification.enabled = true;
      mount();
      assert(control().checked && !control().disabled, "A saved preference cannot be disabled while qualification is unavailable.");
      saveFailure = true;
      control().checked = false;
      control().dispatchEvent(new Event("change", { bubbles: true }));
      await Promise.resolve();
      await Promise.resolve();
      assert(info.settings.cleanup_verification_enabled === true && control().checked && errors.length === 1, "A failed save changed the preference or left a false disabled state.");
      saveFailure = false;
      results.push(`${language}: a failed save reports the error and restores the acknowledged preference.`);
      control().checked = false;
      control().dispatchEvent(new Event("change", { bubbles: true }));
      await Promise.resolve();
      assert(info.settings.cleanup_verification_enabled === false && !control().checked && control().disabled, "Disabling did not save and retain the qualification gate.");
      assert(JSON.stringify([info.settings.dictation_stt, info.settings.meeting_stt, info.settings.llm, info.settings.decision]) === tasks, "Disabling verification changed another provider choice.");
      results.push(`${language}: a saved enabled preference can be disabled without changing provider choices.`);
    }
    assert(calls.length === 0, "Rendering or changing cleanup preferences invoked a provider.");
    results.push("No automatic inference or sample call was made.");
    return { passed: results.length, results, externalServicesUsed: false, userDataUsed: false };
  } finally {
    client.invoke = original;
    target.replaceChildren(...previous);
  }
}
