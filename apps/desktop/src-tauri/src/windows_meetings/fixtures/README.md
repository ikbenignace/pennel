# Mac audio compatibility fixture

`mac16k.m4a` is one second of a generated 440 Hz sine wave at amplitude 0.4.
It contains no microphone or meeting recording. AVFoundation encoded it as
16 kHz mono AAC with a requested bit rate of 32 kb/s, matching the native Mac
meeting recorder's format.

The Windows decoder test reads this fixture through Media Foundation and
checks the resulting 16 kHz mono samples. The Windows encoder test separately
round-trips its 48 kHz AAC output through the same decoder.
