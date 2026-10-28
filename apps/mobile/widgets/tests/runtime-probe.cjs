const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { createRequire } = require('node:module');

const mobileRoot = path.resolve(__dirname, '../..');
const requireMobile = createRequire(path.join(mobileRoot, 'package.json'));
const ts = requireMobile('typescript');
const babel = requireMobile('@babel/core');
const expoRoot = path.dirname(requireMobile.resolve('expo/package.json'));
const sdkRoot = path.dirname(requireMobile.resolve('expo-widgets/package.json'));
const uiRoot = path.dirname(requireMobile.resolve('@expo/ui/package.json'));
const preset = requireMobile.resolve('babel-preset-expo', { paths: [expoRoot] });

function makeRuntime(moduleOverrides = {}) {
  const context = vm.createContext({ console, __DEV__: false });
  const cache = new Map();
  let ui;
  let modifiers;

  function resolveSource(specifier, from) {
    const base = path.resolve(path.dirname(from), specifier);
    const result = [base, `${base}.ts`, `${base}.tsx`, `${base}.js`, `${base}.json`, path.join(base, 'index.ts'), path.join(base, 'index.tsx')]
      .find((candidate) => fs.existsSync(candidate) && fs.statSync(candidate).isFile());
    if (!result) throw new Error(`Probe cannot resolve ${specifier} from ${from}`);
    return result;
  }

  function load(file) {
    if (file.endsWith('.json')) return JSON.parse(fs.readFileSync(file, 'utf8'));
    if (cache.has(file)) return cache.get(file).exports;
    const module = { exports: {} };
    cache.set(file, module);
    const code = ts.transpileModule(fs.readFileSync(file, 'utf8'), {
      fileName: file,
      compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.ReactJSX, esModuleInterop: true },
    }).outputText;
    const localRequire = (name) => {
      if (Object.hasOwn(moduleOverrides, name)) return moduleOverrides[name];
      if (name === 'react') return load(path.join(sdkRoot, 'bundle/react-stub.ts'));
      if (name === 'react/jsx-runtime' || name === 'react/jsx-dev-runtime') return load(path.join(sdkRoot, 'bundle/jsx-runtime-stub.ts'));
      if (name === 'react-native') return load(path.join(sdkRoot, 'bundle/react-native-stub.ts'));
      if (name === 'expo') return load(path.join(sdkRoot, 'bundle/expo-stub.ts'));
      if (name === 'expo-modules-core') return load(path.join(sdkRoot, 'bundle/expo-modules-core-stub.ts'));
      if (name === '@expo/ui/swift-ui') return ui;
      if (name === '@expo/ui/swift-ui/modifiers') return modifiers;
      if (name === './ui-globals' && file === path.join(sdkRoot, 'bundle/index.ts')) return { ...ui, ...modifiers };
      if (name.startsWith('.')) return load(resolveSource(name, file));
      throw new Error(`Unexpected module ${name} in the isolated widget probe.`);
    };
    vm.runInContext(`(function(require,module,exports){${code}\n})`, context, { filename: file })(localRequire, module, module.exports);
    return module.exports;
  }

  ui = {};
  for (const component of ['Text', 'HStack', 'VStack', 'Spacer', 'Link']) {
    Object.assign(ui, load(path.join(uiRoot, `src/swift-ui/${component}/index.tsx`)));
  }
  Object.assign(ui, load(path.join(uiRoot, 'src/swift-ui/Shapes/index.tsx')));
  modifiers = load(path.join(uiRoot, 'src/swift-ui/modifiers/index.ts'));
  load(path.join(sdkRoot, 'bundle/index.ts'));
  return { context, load };
}

function compileLayout(file = path.join(mobileRoot, 'widgets/PennelLiveActivity.tsx')) {
  const result = babel.transformFileSync(file, {
    configFile: false,
    babelrc: false,
    ast: true,
    presets: [[preset, { jsxRuntime: 'classic', disableImportExportTransform: true }]],
    caller: { name: 'metro', platform: 'ios', isDev: false, isServer: false, projectRoot: mobileRoot },
  });
  for (const item of result.ast.program.body) {
    if (item.type !== 'ExportNamedDeclaration' || item.declaration?.type !== 'VariableDeclaration') continue;
    const declaration = item.declaration.declarations.find((entry) => entry.id.name === 'PennelLiveActivity');
    if (declaration?.init?.type === 'TemplateLiteral') return declaration.init.quasis.map((part) => part.value.cooked).join('');
  }
  throw new Error('The SDK Babel widget transform did not serialize PennelLiveActivity.');
}

function renderLayout(props, environment = { colorScheme: 'light' }) {
  const runtime = makeRuntime();
  runtime.context.__expoWidgetLayout = vm.runInContext(`(${compileLayout()})`, runtime.context);
  return JSON.parse(JSON.stringify(runtime.context.__expoWidgetRender(props, environment)));
}

function nodes(value, type) {
  if (value === null || typeof value !== 'object') return [];
  if (Array.isArray(value)) return value.flatMap((child) => nodes(child, type));
  return [...(value.type === type ? [value] : []), ...Object.values(value).flatMap((child) => nodes(child, type))];
}

module.exports = { makeRuntime, compileLayout, renderLayout, nodes, mobileRoot, sdkRoot, uiRoot };
