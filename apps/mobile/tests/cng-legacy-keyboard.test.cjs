const test = require('node:test');
const assert = require('node:assert/strict');
const { removeLegacyKeyboardSources } = require('../plugins/with-pennel.cjs')._test;

test('non-clean generation drops retired keyboard source references while preserving canonical sources and targets', () => {
  const objects = {
    PBXFileReference: {
      OLD_SOURCE: { path: '"../../ios/PennelKeyboard/Sources/KeyboardRoot.swift"', sourceTree: 'SOURCE_ROOT' },
      OLD_SOURCE_comment: 'KeyboardRoot.swift',
      OLD_MODEL: { path: '../../ios/PennelKeyboard/Model/KeyboardModel.swift', sourceTree: 'SOURCE_ROOT' },
      OLD_MODEL_comment: 'KeyboardModel.swift',
      OLD_TEST: { path: '"../../ios/PennelKeyboardTests/KeyboardModelTests.swift"', sourceTree: 'SOURCE_ROOT' },
      OLD_TEST_comment: 'KeyboardModelTests.swift',
      CANONICAL: { path: '"../native/ios/Keyboard/Sources/KeyboardRoot.swift"', sourceTree: 'SOURCE_ROOT' },
      TEMPLATE: { path: '"PennelKeyboard/Info.plist"', sourceTree: 'SOURCE_ROOT' },
      UNRELATED: { path: '"../../other/Sources/KeyboardRoot.swift"', sourceTree: 'SOURCE_ROOT' },
    },
    PBXBuildFile: {
      OLD_SOURCE_BUILD: { fileRef: 'OLD_SOURCE' },
      OLD_SOURCE_BUILD_comment: 'KeyboardRoot.swift in Sources',
      OLD_MODEL_BUILD: { fileRef: 'OLD_MODEL' },
      OLD_MODEL_BUILD_comment: 'KeyboardModel.swift in Sources',
      OLD_TEST_BUILD: { fileRef: 'OLD_TEST' },
      OLD_TEST_BUILD_comment: 'KeyboardModelTests.swift in Sources',
      CANONICAL_BUILD: { fileRef: 'CANONICAL' },
      TEMPLATE_BUILD: { fileRef: 'TEMPLATE' },
      UNRELATED_BUILD: { fileRef: 'UNRELATED' },
    },
    PBXSourcesBuildPhase: {
      KEYBOARD: { files: [{ value: 'OLD_SOURCE_BUILD' }, { value: 'OLD_MODEL_BUILD' }, { value: 'CANONICAL_BUILD' }, { value: 'UNRELATED_BUILD' }] },
      TESTS: { files: [{ value: 'OLD_MODEL_BUILD' }, { value: 'OLD_TEST_BUILD' }] },
    },
    PBXResourcesBuildPhase: { RESOURCES: { files: [{ value: 'TEMPLATE_BUILD' }] } },
    PBXGroup: { NATIVE: { children: [{ value: 'OLD_SOURCE' }, { value: 'OLD_MODEL' }, { value: 'OLD_TEST' }, { value: 'CANONICAL' }, { value: 'TEMPLATE' }, { value: 'UNRELATED' }] } },
    PBXNativeTarget: {
      KEYBOARD_TARGET: { name: 'PennelKeyboard', buildPhases: [{ value: 'KEYBOARD' }, { value: 'RESOURCES' }], dependencies: [] },
      TEST_TARGET: { name: 'PennelKeyboardTests', buildPhases: [{ value: 'TESTS' }], dependencies: [] },
      RUNTIME_TARGET: { name: 'PennelRuntimeTests', dependencies: [{ value: 'HOST_APP' }] },
    },
    XCBuildConfiguration: { RUNTIME_DEBUG: { buildSettings: { TEST_HOST: '"$(BUILT_PRODUCTS_DIR)/Pennel.app/Pennel"', ENABLE_TESTABILITY: 'YES' } } },
  };
  const targets = structuredClone(objects.PBXNativeTarget);
  const configurations = structuredClone(objects.XCBuildConfiguration);
  const project = { hash: { project: { objects } } };
  removeLegacyKeyboardSources(project);
  assert.deepEqual(Object.keys(objects.PBXFileReference), ['CANONICAL', 'TEMPLATE', 'UNRELATED']);
  assert.deepEqual(Object.keys(objects.PBXBuildFile), ['CANONICAL_BUILD', 'TEMPLATE_BUILD', 'UNRELATED_BUILD']);
  assert.deepEqual(objects.PBXSourcesBuildPhase.KEYBOARD.files, [{ value: 'CANONICAL_BUILD' }, { value: 'UNRELATED_BUILD' }]);
  assert.deepEqual(objects.PBXSourcesBuildPhase.TESTS.files, []);
  assert.deepEqual(objects.PBXGroup.NATIVE.children, [{ value: 'CANONICAL' }, { value: 'TEMPLATE' }, { value: 'UNRELATED' }]);
  assert.deepEqual(objects.PBXResourcesBuildPhase.RESOURCES.files, [{ value: 'TEMPLATE_BUILD' }]);
  assert.deepEqual(objects.PBXNativeTarget, targets);
  assert.deepEqual(objects.XCBuildConfiguration, configurations);
  const once = structuredClone(objects);
  removeLegacyKeyboardSources(project);
  assert.deepEqual(objects, once);
});
