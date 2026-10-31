import assert from 'node:assert/strict';
import test from 'node:test';
import { nativeSystemPath } from '../../src/native-routing.ts';

test('the configured sync completion link resolves to the real settings route', () => {
  assert.equal(nativeSystemPath('pennel-preview://sync', ['pennel-preview']), '/settings/sync');
  assert.equal(nativeSystemPath('pennel-preview:///sync', ['pennel-preview']), '/settings/sync');
  assert.equal(nativeSystemPath('pennel://sync/', ['pennel']), '/settings/sync');
});

test('preview does not rewrite another app identity or a web URL', () => {
  for (const path of ['pennel://sync', 'https://pennel.app/sync']) assert.equal(nativeSystemPath(path, ['pennel-preview']), path);
});

test('credentials, queries and fragments never become an accepted completion callback', () => {
  for (const path of ['pennel-preview://user:password@sync', 'pennel-preview://sync?token=fixture', 'pennel-preview://sync#fixture']) {
    assert.equal(nativeSystemPath(path, ['pennel-preview']), path);
  }
});

test('existing meeting paths and malformed inputs retain the router default behavior', () => {
  for (const path of ['pennel-preview:///meeting/fixture', '/settings/appearance', 'not a URL']) assert.equal(nativeSystemPath(path, ['pennel-preview']), path);
});
