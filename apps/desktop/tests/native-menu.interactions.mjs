const assert = (condition, message) => { if (!condition) throw new Error(message); };
const settle = () => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));

/** Runs against the owned browser fixture; it never invokes recording, providers or real native IPC. */
export async function runNativeMenuInteractions() {
  if (!/^(127\.0\.0\.1|localhost)$/.test(location.hostname) || !window.__PENNEL_PLATFORM__ || !window.__PENNEL_FIXTURE__) {
    throw new Error("Menu checks require the owned synthetic fixture server.");
  }
  const { h, select, showPopupMenu } = await import("/src/dom.ts");
  const changes = [];
  const control = select([["alpha", "Alpha"], ["beta", "Beta"], ["gamma", "Gamma"]], "alpha", value => changes.push(value), "menu");
  const owner = h("div", { "data-menu-probe": true }, control);
  document.body.append(owner);
  try {
    control.options[1].disabled = true;
    control.focus(); control.click();
    let list = document.querySelector(".choice-list");
    assert(list && list.getAttribute("role") === "listbox", "Fixture choice did not use its renderer seam.");
    assert(list.querySelector('[aria-selected="true"]').textContent === "Alpha", "Current selection is missing.");
    assert(list.querySelectorAll("button")[1].disabled, "Disabled choice became available.");
    list.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true, cancelable: true }));
    assert(document.activeElement?.textContent === "Gamma", "Arrow navigation did not skip the disabled choice.");
    list.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }));
    assert(!document.querySelector(".choice-list") && document.activeElement === control, "Escape did not return focus.");
    assert(control.value === "alpha" && changes.length === 0, "Cancelling changed the selected value.");

    control.click();
    list = document.querySelector(".choice-list");
    list.dispatchEvent(new KeyboardEvent("keydown", { key: "g", bubbles: true, cancelable: true }));
    assert(document.activeElement?.textContent === "Gamma", "Menu type-to-select did not move focus.");
    document.activeElement.click();
    assert(control.value === "gamma" && changes.join() === "gamma", "A chosen value was not delivered exactly once.");
    control.options[2].label = "Updated gamma";
    assert(control.textContent.includes("Updated gamma"), "The ChoiceElement label contract changed.");

    const failures = [];
    showPopupMenu(control, [{ value: "fail", label: "Save", run: async () => { throw new Error("Synthetic save failure"); } }], { report: error => failures.push(error.message) });
    document.querySelector(".choice-list button").click();
    await Promise.resolve();
    assert(failures.join() === "Synthetic save failure", "Action failures did not reach the owner.");

    let staleActions = 0;
    showPopupMenu(control, [{ value: "stale", label: "Action", run: () => { staleActions++; } }]);
    const staleButton = document.querySelector(".choice-list button");
    owner.remove();
    staleButton.click();
    await settle();
    assert(staleActions === 0 && !document.querySelector(".choice-list"), "Leaving an owner retained its popup or action.");
    return { checks: 12, changes, failures };
  } finally {
    owner.remove();
  }
}
