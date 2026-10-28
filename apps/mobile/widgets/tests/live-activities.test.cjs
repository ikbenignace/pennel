const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const { makeRuntime, compileLayout, renderLayout, nodes, mobileRoot } = require('./runtime-probe.cjs');

const { recordingActivityProps, sessionActivityProps, recordingControlAuthority } = makeRuntime().load(path.join(mobileRoot, 'widgets/props.ts'));
const brand = JSON.parse(fs.readFileSync(path.join(mobileRoot, '../../assets/brand/tokens.json'), 'utf8'));
const started = 1_798_329_600_000;
const observed = started + 65_000;

test('payloads use canonical appearance and contain status rather than meeting content', () => {
  const props = recordingActivityProps({ phase: 'recording', started_at_ms: started, observed_at_ms: observed }, 'en', 'pennel-preview');
  assert.equal(props.appearance.background, brand.colors['pill-background'].light);
  assert.equal(props.appearance.foreground, brand.colors['pill-text'].light);
  assert.equal(props.appearance.wave, brand.colors.wave.light);
  assert.equal(props.appearance.fontSize, brand.typography.sizes.section);
  assert.equal(props.wave_heights.length, 5);
  assert.equal(props.open_url, 'pennel-preview://');
  assert.equal(recordingControlAuthority, 'native-activitykit');
  for (const privateField of ['title', 'text', 'transcript', 'audio', 'token', 'embedding']) assert.equal(privateField in props, false);
});

test('the actual SDK serializes a self-contained widget and renders an elapsed native timer', () => {
  const props = recordingActivityProps({ phase: 'recording', started_at_ms: started, observed_at_ms: observed }, 'en', 'pennel');
  const layout = compileLayout();
  assert.equal(layout.includes('setInterval'), false);
  const rendered = renderLayout(props);
  const clocks = nodes(rendered, 'TextView').filter((node) => node.props.dateStyle === 'timer');
  assert.ok(clocks.length > 0);
  for (const clock of clocks) assert.equal(clock.props.date, started);
  for (const region of ['banner', 'bannerSmall', 'compactLeading', 'compactTrailing', 'minimal', 'expandedLeading', 'expandedCenter', 'expandedTrailing', 'expandedBottom']) {
    assert.ok(Object.hasOwn(rendered, region), region);
  }
  assert.ok(nodes(rendered.banner, 'CapsuleView').length >= 5);
});

test('ready sessions use a bounded native countdown with no JavaScript tick', () => {
  const expires = observed + 300_000;
  const props = sessionActivityProps({ phase: 'ready', expires_at_ms: expires, observed_at_ms: observed }, 'nl', 'pennel-preview');
  const rendered = renderLayout(props);
  const clocks = nodes(rendered, 'TextView').filter((node) => node.props.timerInterval);
  assert.ok(clocks.length > 0);
  for (const clock of clocks) {
    assert.deepEqual(clock.props.timerInterval, { lower: observed, upper: expires });
    assert.equal(clock.props.countsDown, true);
  }
  assert.ok(nodes(rendered, 'TextView').some((node) => node.props.text === 'Pennel staat klaar'));
});

test('expired ready sessions and finishing recordings do not display active timers', () => {
  for (const props of [
    sessionActivityProps({ phase: 'ready', expires_at_ms: observed - 1, observed_at_ms: observed }, 'en', 'pennel'),
    recordingActivityProps({ phase: 'finishing', started_at_ms: started, observed_at_ms: observed }, 'en', 'pennel'),
  ]) {
    assert.equal(props.clock.mode, 'none');
    const rendered = renderLayout(props);
    assert.equal(nodes(rendered, 'TextView').filter((node) => node.props.timerInterval || node.props.date).length, 0);
  }
  const processing = sessionActivityProps({ phase: 'processing', expires_at_ms: observed - 1, observed_at_ms: observed }, 'en', 'pennel');
  assert.equal(processing.labels.status, 'Processing text…');
});

test('stale presentations request verification and expose no misleading Stop action', () => {
  const props = recordingActivityProps({ phase: 'recording', started_at_ms: started, observed_at_ms: observed }, 'nl', 'pennel');
  const rendered = renderLayout(props, { colorScheme: 'dark', isStale: true, isLuminanceReduced: true });
  assert.ok(nodes(rendered, 'TextView').some((node) => node.props.text === props.labels.unavailable));
  assert.equal(nodes(rendered, 'TextView').filter((node) => node.props.timerInterval || node.props.date).length, 0);
  assert.equal(nodes(rendered, 'Button').length, 0);
  const links = nodes(rendered, 'LinkView');
  assert.ok(links.length > 0);
  for (const link of links) assert.equal(link.props.destination, 'pennel://');
});

test('invalid timestamps fail rather than create invalid native timer ranges', () => {
  assert.throws(() => recordingActivityProps({ phase: 'recording', started_at_ms: observed + 1, observed_at_ms: observed }, 'en', 'pennel'), /precedes/);
  assert.throws(() => sessionActivityProps({ phase: 'ready', expires_at_ms: Number.NaN, observed_at_ms: observed }, 'en', 'pennel'), /invalid/);
  assert.throws(() => sessionActivityProps({ phase: 'ready', expires_at_ms: Infinity, observed_at_ms: observed }, 'en', 'pennel'), /invalid/);
});

test('import and explicit registration never start recording or a Live Activity', () => {
  const registered = [];
  const runtime = makeRuntime({
    'expo-widgets': {
      createLiveActivity(name, layout) {
        registered.push({ name, layout });
        return { start() { assert.fail('Prototype registration must not start a Live Activity.'); } };
      },
    },
  });
  const prototypes = runtime.load(path.join(mobileRoot, 'widgets/index.ts'));
  assert.equal(registered.length, 0);
  prototypes.registerLiveActivityPrototypes();
  assert.deepEqual(registered.map((entry) => entry.name), ['PennelRecordingPrototype', 'PennelSessionPrototype']);
});
