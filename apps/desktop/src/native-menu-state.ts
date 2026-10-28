export interface MenuPoint { x: number; y: number }
interface PopupResource { popup(): Promise<void>; close(): Promise<void> }

export function physicalMenuPosition(point: MenuPoint, viewportWidth: number, physicalWidth: number): MenuPoint {
  const scale = viewportWidth > 0 && physicalWidth > 0 ? physicalWidth / viewportWidth : 1;
  return { x: Math.max(0, Math.round(point.x * scale)), y: Math.max(0, Math.round(point.y * scale)) };
}

/** Tauri resolves popup() when shown; its handle must survive until selection or owner disposal. */
export class NativeMenuSession {
  private active = true;
  private resource: PopupResource | null = null;
  private readonly available: () => boolean;
  private readonly report: (error: unknown) => void;

  constructor(available: () => boolean, report: (error: unknown) => void) {
    this.available = available;
    this.report = report;
  }

  async show(create: () => Promise<PopupResource>): Promise<boolean> {
    try {
      const resource = await create();
      this.resource = resource;
      if (!this.active || !this.available()) { this.close(); return false; }
      await resource.popup();
      if (!this.active || !this.available()) { this.close(); return false; }
      return true;
    } catch (error) {
      this.close();
      throw error;
    }
  }

  choose(run: () => unknown): void {
    if (!this.active || !this.available()) { this.close(); return; }
    this.close();
    try { void Promise.resolve(run()).catch(this.report); }
    catch (error) { this.report(error); }
  }

  close(): void {
    this.active = false;
    const resource = this.resource;
    this.resource = null;
    if (resource) {
      try { void resource.close().catch(this.report); }
      catch (error) { this.report(error); }
    }
  }
}
