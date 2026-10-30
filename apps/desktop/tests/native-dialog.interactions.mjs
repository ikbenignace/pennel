const assert = (value, message) => { if (!value) throw new Error(message); };

export async function checkNativeDialogCancellation() {
  const fixture = window.__PENNEL_FIXTURE__;
  assert(fixture?.options.style_picker_cancelled, "The cancellation fixture must be explicit.");
  const button = document.querySelector(".style-add button");
  assert(button && !button.disabled, "The application picker is unavailable.");
  const saves = fixture.calls.filter((command) => command === "save_settings").length;
  button.focus();
  button.click();
  for (let attempt = 0; attempt < 100 && button.disabled; attempt++) await new Promise(resolve => setTimeout(resolve, 20));
  assert(!button.disabled && document.activeElement === button, "Cancelling the native picker lost its invoker focus.");
  assert(fixture.calls.filter((command) => command === "save_settings").length === saves, "Cancelling the native picker changed settings.");
  return { checks: 2, native_sheet_appearance_verified: false };
}
