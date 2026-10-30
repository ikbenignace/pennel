import Constants from 'expo-constants';
import { nativeSystemPath } from '../src/native-routing';

export function redirectSystemPath({ path }: { path: string; initial: boolean }): string {
  const configured = Constants.expoConfig?.scheme;
  const schemes = typeof configured === 'string' ? [configured] : configured ?? [];
  return nativeSystemPath(path, schemes);
}
