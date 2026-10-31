const assert = require('node:assert/strict');
const test = require('node:test');
const { configureTestFlightSigning } = require('../plugins/with-pennel.cjs')._test;

test('TestFlight signing uses the personal team and automatic distribution for Preview', () => {
  const settings = {};
  configureTestFlightSigning(settings, { testFlight: true, production: false });
  assert.equal(settings.DEVELOPMENT_TEAM, '"B538Q2K76C"');
  assert.equal(settings.CODE_SIGN_STYLE, 'Automatic');
});

test('ordinary generated builds retain their existing signing settings', () => {
  const settings = { DEVELOPMENT_TEAM: 'existing', CODE_SIGN_STYLE: 'Manual' };
  configureTestFlightSigning(settings, { production: false });
  assert.deepEqual(settings, { DEVELOPMENT_TEAM: 'existing', CODE_SIGN_STYLE: 'Manual' });
});

test('Preview distribution cannot select production identities', () => {
  assert.throws(() => configureTestFlightSigning({}, { testFlight: true, production: true }), /Preview/);
});
