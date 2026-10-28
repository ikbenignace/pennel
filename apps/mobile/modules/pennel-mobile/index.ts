import { requireOptionalNativeModule } from 'expo-modules-core';
import type { MobileCommand, MobileCommandResults, RuntimeSnapshot } from './src/types';
export * from './src/types';

interface Subscription { remove(): void }
interface NativeRuntime {
  bootstrap(): Promise<string>;
  getSnapshot(): Promise<string>;
  invoke(method: string, payload: string): Promise<string>;
  addListener(name: 'snapshot', listener: (event: { snapshot: string }) => void): Subscription;
}

const native = requireOptionalNativeModule<NativeRuntime>('PennelMobile');

function runtime(): NativeRuntime {
  if (!native) throw new Error('Pennel requires its native development build. Run the configured iOS build command.');
  return native;
}

export const PennelMobile = {
  available: native !== null,
  async bootstrap(): Promise<RuntimeSnapshot> {
    return JSON.parse(await runtime().bootstrap()) as RuntimeSnapshot;
  },
  async getSnapshot(): Promise<RuntimeSnapshot> {
    return JSON.parse(await runtime().getSnapshot()) as RuntimeSnapshot;
  },
  subscribe(listener: (snapshot: RuntimeSnapshot) => void): () => void {
    const subscription = runtime().addListener('snapshot', event => listener(JSON.parse(event.snapshot) as RuntimeSnapshot));
    return () => subscription.remove();
  },
  async invoke<T = unknown>(method: string, payload: Record<string, unknown> = {}): Promise<T> {
    return JSON.parse(await runtime().invoke(method, JSON.stringify(payload))) as T;
  },
  async command<K extends MobileCommand>(method: K, payload: Record<string, unknown> = {}): Promise<MobileCommandResults[K]> {
    return JSON.parse(await runtime().invoke(method, JSON.stringify(payload))) as MobileCommandResults[K];
  },
};

export default PennelMobile;
