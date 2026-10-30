export async function checkStartupLoading() {
  if (!window.__PENNEL_FIXTURE__) throw new Error("Synthetic fixture required");
  const status = document.querySelector(".startup-loading[role=status]");
  if (!status || !status.textContent.trim()) throw new Error("Slow startup must show a loading state");
  for (let i = 0; i < 150; i++) {
    if (document.querySelector(".library-sidebar")) return { loading: true, recovered: true };
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  throw new Error("Workspace did not appear after startup completed");
}

export async function checkStartupError() {
  if (!window.__PENNEL_FIXTURE__) throw new Error("Synthetic fixture required");
  for (let i = 0; i < 100; i++) {
    const error = document.querySelector(".startup-error[role=alert]");
    if (error) {
      if (!document.querySelector(".startup-error button")) throw new Error("Startup error must offer retry");
      return { error: true, retry: true };
    }
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  throw new Error("Startup failure must be visible");
}
