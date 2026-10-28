import { createLiveActivity } from 'expo-widgets';

import { PennelLiveActivity } from './PennelLiveActivity';
import type { RecordingActivityProps, SessionActivityProps } from './types';

export { PennelLiveActivity } from './PennelLiveActivity';
export { recordingActivityProps, sessionActivityProps, recordingControlAuthority } from './props';
export type * from './types';

export function registerLiveActivityPrototypes() {
  return {
    recording: createLiveActivity<RecordingActivityProps>('PennelRecordingPrototype', PennelLiveActivity),
    session: createLiveActivity<SessionActivityProps>('PennelSessionPrototype', PennelLiveActivity),
  };
}
