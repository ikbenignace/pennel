export async function checkInferenceUI() {
  if (!/^(127\.0\.0\.1|localhost)$/.test(location.hostname) || !window.__PENNEL_FIXTURE__ || !window.__PENNEL_PLATFORM__) throw new Error("Use the owned synthetic fixture page.");
  const { inferenceModelsView } = await import("../src/views/inference.ts");
  const { setInterfaceLanguage, getInterfaceLanguage } = await import("../src/i18n.ts");
  const client = window.__PENNEL_PLATFORM__;
  const original = client.invoke.bind(client);
  const info = window.__PENNEL_FIXTURE__.info();
  info.settings.providers.push({ id: "typed-fixture", name: "Synthetic decision server", kind: "custom", base_url: "https://decision.example/v1" });
  info.keys["typed-fixture"] = true;
  info.provider_revisions = { "typed-fixture": 1 };
  info.settings.decision = null; info.settings.reranking = null;
  const originalChoices = JSON.stringify([info.settings.dictation_stt, info.settings.meeting_stt, info.settings.llm]);
  const operations = [];
  const protocols = [
    { protocol: "type_safe", name: "TypeSafe System One", default_base_url: "https://api.typesafe.ai/v1", decision: true, reranking: false, extraction: false, requires_key: true },
    { protocol: "tei", name: "TEI", default_base_url: "http://127.0.0.1:8080", decision: false, reranking: true, extraction: false, requires_key: false },
  ];
  const identity = { protocol: "type_safe", endpoint: "https://decision.example/v1/systemone", requested_model: "fixture-model", requested_upstream: null, actual_model: "fixture-model-v1", actual_provider: "Fixture", actual_revision: "revision-1", request_id: "fixture-request" };
  let responder = async () => ({ verified: true, message: "Sample passed", identity });
  client.invoke = async (command, args) => {
    if (command === "inference_protocols") return protocols;
    if (command === "inference_availability") {
      const config = info.settings[args.capability];
      return { capability: args.capability, configured: !!config, available: !!config, reason: config ? "Configured" : "Not configured", provider_id: config?.provider_id ?? null, model: config?.model ?? null, protocol: config?.protocol ?? null, endpoint: config ? identity.endpoint : null };
    }
    if (command === "verify_inference") { operations.push({ command, ...args }); return responder(args); }
    if (command === "cancel_inference") { operations.push({ command, ...args }); return true; }
    return original(command, args);
  };
  const target = document.getElementById("content");
  const previous = [...target.childNodes];
  const previousLanguage = getInterfaceLanguage();
  let cleanup = [], listeners = [], pendingMount = Promise.resolve();
  const errors = [];
  let saveFailure = null;
  const context = {
    get info() { return info; },
    async save(change) { if (saveFailure) throw saveFailure; change(info.settings); for (const listener of [...listeners]) listener(info); },
    onDispose(action) { cleanup.push(action); },
    onInfoChange(listener) { listeners.push(listener); },
    rerender() { pendingMount = pendingMount.then(mount); },
    navigate() {}, report(error) { errors.push(String(error)); },
  };
  async function mount() {
    cleanup.forEach(action => action()); cleanup = []; listeners = [];
    target.replaceChildren(...await inferenceModelsView(context));
  }
  const settle = async () => { await new Promise(resolve => setTimeout(resolve, 0)); await pendingMount; await new Promise(resolve => setTimeout(resolve, 0)); };
  const assert = (condition, message) => { if (!condition) throw new Error(message); };
  const decision = () => target.querySelector('[data-inference-capability="decision"]');
  const button = name => [...decision().querySelectorAll("button")].find(item => item.textContent.trim() === name);
  const results = [];
  try {
    setInterfaceLanguage("en"); await mount();
    assert(operations.length === 0 && decision().textContent.includes("Not configured"), "Mounting enabled or tested an optional connection.");
    const model = decision().querySelector('[data-inference-field="model"]'); model.value = "fixture-model"; model.dispatchEvent(new Event("input", { bubbles: true }));
    button("Save configuration").click(); await settle();
    assert(info.settings.decision?.model === "fixture-model", "Explicit configuration was not saved.");
    assert(JSON.stringify([info.settings.dictation_stt, info.settings.meeting_stt, info.settings.llm]) === originalChoices, "Existing STT/LLM selections changed.");
    assert(operations.length === 0, "Saving configuration ran inference automatically.");
    results.push("Optional configuration saves atomically without changing or invoking existing tasks.");
    button("Test with sample").click(); await settle();
    assert(decision().textContent.includes("The sample test passed.") && decision().textContent.includes("fixture-model-v1") && decision().textContent.includes("revision-1"), "Sample outcome or actual identity missing.");
    results.push("Explicit sample verification exposes actual identity and revision.");
    let finish;
    responder = () => new Promise(resolve => { finish = resolve; });
    button("Test with sample").click(); await settle();
    info.provider_revisions["typed-fixture"]++;
    for (const listener of [...listeners]) listener(info);
    await settle(); finish({ verified: true, message: "obsolete", identity: { ...identity, request_id: "obsolete-result" } }); await settle();
    assert(!decision().textContent.includes("The sample test passed.") && !decision().textContent.includes("obsolete-result") && operations.some(item => item.command === "cancel_inference"), "A late response verified changed credentials.");
    results.push("Credential changes cancel verification and suppress stale outcomes.");
    button("Test with sample").click(); await settle();
    button("Cancel").click(); await settle();
    assert(!decision().textContent.includes("Processing the sample…"), "Cancelled verification retained an active-processing message.");
    finish({ verified: true, message: "late", identity }); await settle();
    assert(!decision().textContent.includes("The sample test passed."), "Cancelled verification produced a late success.");
    results.push("Cancelling discards an already-running sample result.");
    const edited = decision().querySelector('[data-inference-field="model"]'); edited.value = "not-saved"; edited.dispatchEvent(new Event("input", { bubbles: true }));
    assert(button("Test with sample").disabled && info.settings.decision.model === "fixture-model", "Unsaved input changed the selected provider or remained testable.");
    saveFailure = new Error("Synthetic storage failure"); button("Save configuration").click(); await settle();
    assert(decision().textContent.includes("Synthetic storage failure") && info.settings.decision.model === "fixture-model", "A failed save lost its explanation or replaced the acknowledged configuration.");
    saveFailure = null;
    setInterfaceLanguage("nl");
    assert(decision().textContent.includes("Beslissingen verwerken") && button("Configuratie bewaren"), "Dutch presentation did not relabel the controls.");
    setInterfaceLanguage("en");
    button("Disable").click(); await settle();
    assert(info.settings.decision === null && JSON.stringify([info.settings.dictation_stt, info.settings.meeting_stt, info.settings.llm]) === originalChoices, "Disabling changed another task or selected a fallback.");
    assert(errors.length === 0, errors.join("\n"));
    results.push("Unsaved changes, live EN/NL labels and disabling preserve task ownership.");
    return { passed: results.length, results, externalServicesUsed: false, userDataUsed: false };
  } finally {
    cleanup.forEach(action => action()); client.invoke = original; setInterfaceLanguage(previousLanguage); target.replaceChildren(...previous);
  }
}
