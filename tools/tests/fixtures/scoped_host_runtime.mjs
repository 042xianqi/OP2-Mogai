import { readFileSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';
import vm from 'node:vm';

/** Installs production host ownership/snapshot semantics and the shared SDK Promise helper. */
export function installScopedHostRuntime(sandbox) {
  vm.runInContext(`
    globalThis.__operitCurrentCallId = 'owner';
    globalThis.refs = 0;
    globalThis.__operitRetainCallReference = function() { refs++; };
    globalThis.__operitReleaseCallReference = function() { refs--; };
    globalThis.__operitActivateCall = function(id) { __operitCurrentCallId = id; };
    globalThis.__operitExpose = function(name, value) { globalThis[name] = value; };
  `, sandbox);
  for (const path of [
    '../../../hosts/web/src/javascript_promises.js',
    '../../../core/crates/plugin/sdk/src/JsExecutionRuntimeBridge.script.js',
  ]) vm.runInContext(readFileSync(new URL(path, import.meta.url), 'utf8'), sandbox);
  return sandbox.__operitHostPromiseRegistry;
}

let generatedTools;

/** Generates the actual current Tools runtime instead of reimplementing generator templates in tests. */
export function generatedToolsSource() {
  if (generatedTools !== undefined) return generatedTools;
  const root = fileURLToPath(new URL('../../../', import.meta.url));
  const output = join(mkdtempSync(join(tmpdir(), 'operit-tools-contract-')), 'js_tools.js');
  execFileSync('cargo', ['run', '--quiet', '--manifest-path', 'core/Cargo.toml', '--package', 'operit-plugin-sdk-codegen', '--', 'generate-runtime', 'core/crates/plugin/sdk/src', output], { cwd: root, stdio: ['ignore', 'pipe', 'pipe'] });
  generatedTools = readFileSync(output, 'utf8');
  return generatedTools;
}

/** Installs official version gates and freshly generated Tools over the production structured transport. */
export function installGeneratedToolsRuntime(sandbox, version = '2.0.0') {
  sandbox.version = version;
  sandbox.__operitGetCallState = () => ({ params: { __operit_toolpkg_api_version: sandbox.version } });
  const api = readFileSync(new URL('../../../core/crates/plugin/sdk/src/toolpkg/ToolPkgApiRuntimeScript.rs', import.meta.url), 'utf8');
  const start = api.indexOf('r#"') + 3;
  vm.runInContext(api.slice(start, api.indexOf('"#', start)), sandbox);
  vm.runInContext(generatedToolsSource(), sandbox);
}
