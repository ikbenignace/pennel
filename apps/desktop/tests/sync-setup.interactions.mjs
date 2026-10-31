/** Run only in the owned fixture; every account, code and meeting here is synthetic. */
const assert = (value, message) => { if (!value) throw new Error(message); };
const visible = node => node?.getClientRects().length && !node.closest("details:not([open])");
const button = label => [...document.querySelectorAll("button")].find(node => visible(node) && node.textContent.trim() === label);
async function until(check, message) {
  for (let attempt = 0; attempt < 100; attempt++) {
    if (check()) return;
    await new Promise(resolve => setTimeout(resolve, 40));
  }
  throw new Error(message);
}
function fixture() {
  assert(/^(127\.0\.0\.1|localhost)$/.test(location.hostname) && window.__PENNEL_FIXTURE__, "Synthetic fixture required.");
  return window.__PENNEL_FIXTURE__;
}

export async function checkSyncSetup() {
  const state = fixture();
  const { t } = await import("/src/i18n.ts");
  const resume = button(t("account.setup.resume"));
  assert(resume && !resume.disabled, "A validated paused library has no Resume action.");
  assert(!document.querySelector("input[aria-label='Recovery code']"), "Resume unexpectedly requires recovery entry.");
  resume.click();
  await until(() => button(t("account.setup.pause")), "Resume did not activate the sync controls.");
  assert(state.calls.includes("cloud_sync_resume"), "Resume used the old create-vault operation.");
  assert(!state.calls.includes("cloud_sync_enable"), "Resume created a new library.");
  assert(!state.unknown.length, `Unknown commands: ${state.unknown.join(", ")}`);
}

export async function checkDeviceApproval() {
  const state = fixture();
  const { t } = await import("/src/i18n.ts");
  const approve = button(t("account.link.approve"));
  assert(approve && !approve.disabled, "Explicit device approval is missing.");
  assert(document.body.textContent.includes("481 926"), "The verification code is missing.");
  assert(!state.calls.includes("cloud_link_approve"), "Approval happened before a user action.");
  approve.click();
  await until(() => document.body.textContent.includes(t("account.link.phase.complete")), "Approved connection did not show its result.");
  assert(state.calls.filter(command => command === "cloud_link_approve").length === 1, "Approval was dispatched more than once.");
  assert(!state.unknown.length, `Unknown commands: ${state.unknown.join(", ")}`);
}

export async function checkFirstSyncBackup() {
  const state = fixture();
  const { t } = await import("/src/i18n.ts");
  const prepare = button(t("account.setup.create"));
  assert(prepare && !prepare.disabled, "First-device sync setup is missing.");
  prepare.click();
  await until(() => button(t("account.recovery.saved")), "The recovery backup step did not open.");
  assert(state.info().settings.cloud_sync === false, "Sync started before backup acknowledgment.");
  assert(state.calls.includes("cloud_sync_prepare") && !state.calls.includes("cloud_recovery_confirm"), "Backup preparation activated sync.");
  button(t("account.recovery.saved")).click();
  await until(() => button(t("account.setup.pause")), "Acknowledging the backup did not activate sync.");
  assert(state.calls.filter(command => command === "cloud_recovery_confirm").length === 1, "Activation was not dispatched once.");
  assert(!state.unknown.length, `Unknown commands: ${state.unknown.join(", ")}`);
}

export async function checkLateRecoveryAfterLogout() {
  const state = fixture();
  const { t } = await import("/src/i18n.ts");
  button(t("account.setup.create")).click();
  await until(() => state.calls.includes("cloud_sync_prepare"), "Recovery preparation did not start.");
  document.querySelector(".account-management").open = true;
  const logout = button(t("account.logout.action"));
  assert(logout, "Sign-out is unavailable while preparation is pending.");
  logout.click();
  await until(() => button(t("account.setup.email")), "Sign-out did not reach setup.");
  await new Promise(resolve => setTimeout(resolve, 900));
  assert(!document.querySelector(".recovery-code"), "A late recovery response restored a signed-out secret.");
  assert(!state.calls.includes("cloud_recovery_code_context"), "The signed-out response continued into recovery context validation.");
  assert(!localStorage.getItem("pennel.recoveryCodeConfirmed"), "A stale operation restored acknowledgment bookkeeping after sign-out.");
  assert(!state.unknown.length, `Unknown commands: ${state.unknown.join(", ")}`);
}

export async function checkRecoveryRotation() {
  const state = fixture();
  const { t } = await import("/src/i18n.ts");
  const { api } = await import("/src/api.ts");
  button(t("account.setup.create")).click();
  await until(() => button(t("account.recovery.saved")), "The first recovery code was not displayed.");
  await api.cloudSyncNewRecoveryCode();
  button(t("account.recovery.saved")).click();
  await until(() => button(t("account.setup.replace_backup")), "An outdated displayed recovery code was not rejected.");
  assert(state.info().settings.cloud_sync === false, "Acknowledging an outdated code activated sync.");
  assert(localStorage.getItem("pennel.recoveryCodeConfirmed") === "false", "An outdated code was marked saved.");
  assert(!document.querySelector(".recovery-code"), "The invalidated recovery code remains displayed.");
  assert(!state.unknown.length, `Unknown commands: ${state.unknown.join(", ")}`);
}
