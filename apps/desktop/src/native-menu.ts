import { isTauri } from "@tauri-apps/api/core";
import { PhysicalPosition } from "@tauri-apps/api/dpi";
import { Menu, type MenuItemOptions, type CheckMenuItemOptions, type PredefinedMenuItemOptions } from "@tauri-apps/api/menu";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { fixtureClient } from "./api";
import { NativeMenuSession, physicalMenuPosition, type MenuPoint } from "./native-menu-state.ts";

export interface NativePopupItem {
  value: string;
  label: string;
  disabled?: boolean;
  separatorBefore?: boolean;
  run: () => unknown;
}

export function nativeMenusAvailable(): boolean {
  return !fixtureClient() && isTauri();
}

export function popupOwnerAvailable(anchor: HTMLElement): boolean {
  return anchor.isConnected && !anchor.matches(":disabled") && !anchor.closest("[hidden],[inert]") && document.visibilityState !== "hidden";
}

let sequence = 0;

export function showNativePopup(
  anchor: HTMLElement,
  items: NativePopupItem[],
  options: { current?: string; position?: MenuPoint; selected: () => void; report: (error: unknown) => void },
): { close: () => void; ready: Promise<boolean> } {
  const id = ++sequence;
  const session = new NativeMenuSession(() => popupOwnerAvailable(anchor), options.report);
  const ready = session.show(async () => {
    const bounds = anchor.getBoundingClientRect();
    const size = await getCurrentWindow().innerSize();
    const point = physicalMenuPosition(options.position ?? { x: bounds.left, y: bounds.bottom + 4 }, innerWidth, size.width);
    const entries: (MenuItemOptions | CheckMenuItemOptions | PredefinedMenuItemOptions)[] = [];
    for (const [index, item] of items.entries()) {
      if (item.separatorBefore && index > 0) entries.push({ item: "Separator" });
      entries.push({
        id: `pennel-popup-${id}-${index}`,
        text: item.label.replaceAll("&", "&&"),
        enabled: !item.disabled,
        ...(options.current !== undefined ? { checked: item.value === options.current } : {}),
        action: () => {
          if (item.disabled) return;
          session.choose(() => { options.selected(); return item.run(); });
        },
      });
    }
    const menu = await Menu.new({ items: entries });
    return { popup: () => menu.popup(new PhysicalPosition(point.x, point.y)), close: () => menu.close() };
  });
  return { close: () => session.close(), ready };
}
