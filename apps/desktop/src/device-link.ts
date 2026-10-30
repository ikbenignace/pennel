import type { CloudLinkPhase, CloudLinkRole, CloudSyncSetup } from "./api.ts";

export function linkIsPending(phase: CloudLinkPhase): boolean {
  return ["waiting_peer", "confirming", "awaiting_approval", "waiting_approval", "provisioning"].includes(phase);
}

export function connectionRole(setup: CloudSyncSetup): CloudLinkRole {
  return setup.can_approve_devices ? "trusted_device" : "new_device";
}

export function syncSetupAction(setup: CloudSyncSetup): "sign_in" | "create" | "unlock" | "resume" | "active" {
  switch (setup.phase) {
    case "signed_out": return "sign_in";
    case "new_library": return "create";
    case "locked": return "unlock";
    case "ready":
    case "paused": return "resume";
    case "active": return "active";
  }
}

export function syncRequiresBackup(setup: CloudSyncSetup, confirmed: boolean | null): boolean {
  return setup.phase === "ready" || (!setup.sync_enabled && setup.phase === "paused" && confirmed === false);
}

/** The QR is a native-generated image, never active markup from the relay. */
export function qrImageSource(svg: string | null): string | null {
  if (!svg || svg.length > 262_144 || !/^<svg(?:\s|>)/.test(svg.trim())) return null;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}
