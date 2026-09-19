#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { createReadStream, existsSync } from 'node:fs';
import { chmod, mkdir, mkdtemp, readFile, readdir, rename, rm, stat, writeFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawn } from 'node:child_process';

const FORMAT_VERSION = 1;
const DOWNLOAD_INSTRUCTION = 'Download pretrained model:';
const REPOSITORY_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
export const EXPECTED_NO_MODEL_ROUTES = new Set([
  '/vendor/models/catalog.json',
  '/vendor/graph/manifest.json',
  '/vendor/graph/neurons.bin',
  '/vendor/graph/geometry_lod0.bin',
  '/vendor/brain/metadata.json',
]);

export function parseArgs(argv) {
  const args = { timeoutMs: 45_000 };
  for (let index = 0; index < argv.length; index += 1) {
    const option = argv[index];
    const value = argv[index + 1];
    if (option === '--archive' && value) args.archive = value;
    else if (option === '--evidence' && value) args.evidence = value;
    else if (option === '--browser' && value) args.browser = value;
    else if (option === '--timeout-ms' && value) args.timeoutMs = Number(value);
    else throw new Error(`Unknown or incomplete option: ${option}`);
    index += 1;
  }
  if (!args.archive) throw new Error('--archive is required');
  if (!args.evidence) throw new Error('--evidence is required');
  if (!Number.isSafeInteger(args.timeoutMs) || args.timeoutMs < 1_000) {
    throw new Error('--timeout-ms must be an integer of at least 1000');
  }
  return args;
}

async function walkFiles(root) {
  const files = [];
  for (const entry of await readdir(root, { withFileTypes: true })) {
    const candidate = path.join(root, entry.name);
    if (entry.isDirectory()) files.push(...await walkFiles(candidate));
    else if (entry.isFile()) files.push(candidate);
  }
  return files;
}

export async function resolveArchive(candidate) {
  const resolved = path.resolve(candidate);
  const info = await stat(resolved);
  if (info.isFile()) {
    if (path.extname(resolved).toLowerCase() !== '.zip') throw new Error(`Portable archive is not a ZIP: ${resolved}`);
    return resolved;
  }
  if (!info.isDirectory()) throw new Error(`Portable archive path is neither a file nor directory: ${resolved}`);
  const archives = (await walkFiles(resolved)).filter(file => path.extname(file).toLowerCase() === '.zip');
  if (archives.length !== 1) throw new Error(`Expected exactly one ZIP below ${resolved}, found ${archives.length}`);
  return archives[0];
}

function collectOutput(stream) {
  let output = '';
  stream?.setEncoding('utf8');
  stream?.on('data', chunk => {
    output = `${output}${chunk}`.slice(-32_768);
  });
  return () => output.trim();
}

function run(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { ...options, stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true });
    const stdout = collectOutput(child.stdout);
    const stderr = collectOutput(child.stderr);
    child.once('error', reject);
    child.once('exit', code => {
      if (code === 0) resolve({ stdout: stdout(), stderr: stderr() });
      else reject(new Error(`${command} exited ${code}: ${stderr() || stdout()}`));
    });
  });
}

function spawnManaged(command, args, options = {}) {
  const child = spawn(command, args, { ...options, stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true });
  let spawnError;
  let resolveStarted;
  let rejectStarted;
  const started = new Promise((resolve, reject) => {
    resolveStarted = resolve;
    rejectStarted = reject;
  });
  child.once('spawn', resolveStarted);
  // Keep a permanent listener: a later process error must never become an
  // unhandled EventEmitter error after the initial spawn promise has settled.
  child.on('error', error => {
    spawnError = error;
    rejectStarted(error);
  });
  return { child, started, get spawnError() { return spawnError; } };
}

function delay(milliseconds) {
  return new Promise(resolve => setTimeout(resolve, milliseconds));
}

function remainingMs(deadline, description) {
  const remaining = deadline - Date.now();
  if (remaining <= 0) throw new Error(`Timed out waiting for ${description}`);
  return remaining;
}

async function withDeadline(promise, deadline, description, onTimeout) {
  const milliseconds = remainingMs(deadline, description);
  let timer;
  try {
    return await Promise.race([
      promise,
      new Promise((resolve, reject) => {
        timer = setTimeout(() => {
          onTimeout?.();
          reject(new Error(`Timed out waiting for ${description}`));
        }, milliseconds);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

async function waitFor(predicate, deadline, description, retryErrors = true) {
  let lastError;
  while (Date.now() < deadline) {
    try {
      const value = await withDeadline(Promise.resolve().then(predicate), deadline, description);
      if (value) return value;
    } catch (error) {
      if (!retryErrors) throw error;
      lastError = error;
    }
    await delay(Math.min(100, Math.max(1, deadline - Date.now())));
  }
  throw new Error(`Timed out waiting for ${description}${lastError ? `: ${lastError.message}` : ''}`);
}

async function fetchWithDeadline(url, options, deadline, description) {
  const controller = new AbortController();
  const milliseconds = remainingMs(deadline, description);
  const timer = setTimeout(() => controller.abort(), milliseconds);
  try {
    return await fetch(url, { ...options, signal: controller.signal });
  } catch (error) {
    if (controller.signal.aborted) throw new Error(`Timed out waiting for ${description}`);
    throw error;
  } finally {
    clearTimeout(timer);
  }
}

async function availablePort() {
  const socket = createServer();
  await new Promise((resolve, reject) => {
    socket.once('error', reject);
    socket.listen(0, '127.0.0.1', resolve);
  });
  const address = socket.address();
  await new Promise((resolve, reject) => socket.close(error => error ? reject(error) : resolve()));
  if (!address || typeof address === 'string') throw new Error('Could not allocate a loopback port');
  return address.port;
}

async function sha256(file) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(file)) hash.update(chunk);
  return hash.digest('hex');
}

async function extractArchive(archive, destination) {
  const python = process.platform === 'win32' ? 'python' : 'python3';
  await run(python, ['-m', 'zipfile', '-e', archive, destination]);
  const bundle = path.join(destination, 'stockfly');
  if (!(await stat(bundle)).isDirectory()) throw new Error('Portable ZIP does not contain the stockfly root directory');
  return bundle;
}

function selectedHeaders(response) {
  return Object.fromEntries(['content-type', 'cache-control', 'cross-origin-opener-policy', 'cross-origin-embedder-policy']
    .map(name => [name, response.headers.get(name)])
    .filter(([, value]) => value !== null));
}

export async function checkRoute(baseUrl, route, expectedStatus, validate, deadline = Date.now() + 10_000) {
  const response = await fetchWithDeadline(`${baseUrl}${route}`, { redirect: 'manual' }, deadline, `${route} response`);
  const body = await withDeadline(response.text(), deadline, `${route} body`);
  if (response.status !== expectedStatus) {
    throw new Error(`${route} returned HTTP ${response.status}; expected ${expectedStatus}`);
  }
  validate(body, response);
  return { route, status: response.status, headers: selectedHeaders(response), bytes: Buffer.byteLength(body) };
}

function assertIsolationHeaders(response, route) {
  if (response.headers.get('cross-origin-opener-policy') !== 'same-origin'
      || response.headers.get('cross-origin-embedder-policy') !== 'require-corp') {
    throw new Error(`${route} is missing required cross-origin isolation headers`);
  }
}

function browserCandidates() {
  if (process.platform === 'win32') {
    const programFiles = [process.env.PROGRAMFILES, process.env['PROGRAMFILES(X86)'], process.env.LOCALAPPDATA].filter(Boolean);
    return programFiles.flatMap(root => [
      path.join(root, 'Google/Chrome/Application/chrome.exe'),
      path.join(root, 'Chromium/Application/chrome.exe'),
      path.join(root, 'Microsoft/Edge/Application/msedge.exe'),
    ]);
  }
  if (process.platform === 'darwin') {
    return [
      '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      '/Applications/Chromium.app/Contents/MacOS/Chromium',
      '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
    ];
  }
  return ['/usr/bin/chromium', '/usr/bin/chromium-browser', '/usr/bin/google-chrome', '/usr/bin/microsoft-edge'];
}

export function resolveBrowser(explicitBrowser) {
  if (explicitBrowser) {
    const resolved = path.resolve(explicitBrowser);
    if (!existsSync(resolved)) throw new Error(`Chromium browser does not exist: ${resolved}`);
    return resolved;
  }
  const browser = browserCandidates().find(existsSync);
  if (!browser) throw new Error('No supported Chromium browser executable was found');
  return browser;
}

export class CdpSession {
  constructor(url, onEvent, deadline, socketFactory = address => new WebSocket(address)) {
    this.url = url;
    this.onEvent = onEvent;
    this.deadline = deadline;
    this.socketFactory = socketFactory;
    this.nextId = 1;
    this.pending = new Map();
    this.closed = new Promise(resolve => { this.resolveClosed = resolve; });
  }

  async connect() {
    this.socket = this.socketFactory(this.url);
    this.socket.onmessage = event => {
      let message;
      try {
        message = JSON.parse(event.data);
      } catch (error) {
        this.disconnect(new Error(`Invalid Chromium DevTools message: ${error.message}`));
        return;
      }
      if (message.id) {
        const pending = this.pending.get(message.id);
        if (!pending) return;
        this.pending.delete(message.id);
        clearTimeout(pending.timer);
        if (message.error) pending.reject(new Error(message.error.message));
        else pending.resolve(message.result);
      } else {
        try {
          this.onEvent(message.method, message.params ?? {});
        } catch (error) {
          this.disconnect(error instanceof Error ? error : new Error(String(error)));
        }
      }
    };
    const connected = new Promise((resolve, reject) => {
      this.rejectConnect = reject;
      this.socket.onopen = resolve;
      this.socket.onerror = () => this.disconnect(new Error(`Chromium DevTools websocket error at ${this.url}`));
      this.socket.onclose = () => {
        this.disconnect(new Error(`Chromium DevTools websocket closed at ${this.url}`));
        this.resolveClosed();
      };
    });
    await withDeadline(connected, this.deadline, 'Chromium DevTools websocket connection', () => {
      this.disconnect(new Error('Chromium DevTools websocket connection timed out'));
      try { this.socket.close(); } catch {}
    });
    this.rejectConnect = undefined;
  }

  send(method, params = {}) {
    if (this.disconnectError) return Promise.reject(this.disconnectError);
    let milliseconds;
    try {
      milliseconds = remainingMs(this.deadline, `Chromium DevTools command ${method}`);
    } catch (error) {
      return Promise.reject(error);
    }
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`Timed out waiting for Chromium DevTools command ${method}`));
      }, milliseconds);
      this.pending.set(id, { resolve, reject, timer });
      try {
        this.socket.send(JSON.stringify({ id, method, params }));
      } catch (error) {
        clearTimeout(timer);
        this.pending.delete(id);
        reject(error);
      }
    });
  }

  disconnect(error) {
    if (this.disconnectError) return;
    this.disconnectError = error;
    this.rejectConnect?.(error);
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timer);
      pending.reject(error);
    }
    this.pending.clear();
  }

  async close() {
    if (!this.socket) return;
    this.disconnect(new Error('Chromium DevTools session closed'));
    if (this.socket.readyState === 3) {
      this.resolveClosed();
      return;
    }
    try { this.socket.close(); } catch { return; }
    await withDeadline(this.closed, this.deadline, 'Chromium DevTools websocket close');
  }
}

function remoteValue(argument) {
  if ('value' in argument) return String(argument.value);
  return argument.description ?? argument.type ?? '';
}

function expectedNoModelUrl(url, baseUrl) {
  try {
    const parsed = new URL(url);
    return parsed.origin === baseUrl && EXPECTED_NO_MODEL_ROUTES.has(parsed.pathname);
  } catch {
    return false;
  }
}

export function isExpectedNoModelHttpError(item, baseUrl) {
  return item?.status === 404 && expectedNoModelUrl(item.url, baseUrl);
}

export function isExpectedNoModelLog(entry, expectedRequestIds = new Set(), baseUrl = '') {
  return entry?.source === 'network'
    && entry?.level === 'error'
    && /404/.test(entry.text ?? '')
    && (expectedNoModelUrl(entry.url, baseUrl) || expectedRequestIds.has(entry.networkRequestId));
}

async function browserSmoke(browserExecutable, baseUrl, workRoot, timeoutMs) {
  const profile = path.join(workRoot, 'chromium-profile');
  await mkdir(profile);
  const deadline = Date.now() + timeoutMs;
  const activePortFile = path.join(profile, 'DevToolsActivePort');
  let browser;
  let browserStdout = () => '';
  let browserStderr = () => '';
  let session;
  const pageErrors = [];
  const consoleErrors = [];
  const logErrors = [];
  const failedRequests = [];
  const httpErrors = [];
  let version;
  let state;
  let result;
  let failure;
  try {
    const managedBrowser = spawnManaged(browserExecutable, [
      '--headless=new',
      '--remote-debugging-address=127.0.0.1',
      '--remote-debugging-port=0',
      `--user-data-dir=${profile}`,
      '--no-first-run',
      '--no-default-browser-check',
      '--disable-background-networking',
      '--disable-component-update',
      'about:blank',
    ]);
    browser = managedBrowser.child;
    browserStdout = collectOutput(browser.stdout);
    browserStderr = collectOutput(browser.stderr);
    await withDeadline(managedBrowser.started, deadline, 'headless Chromium process start', () => browser.kill());
    const devtoolsPort = await waitFor(async () => {
      const contents = await readFile(activePortFile, 'utf8');
      return Number(contents.split(/\r?\n/, 1)[0]) || null;
    }, deadline, 'Chromium DevTools endpoint');
    const targetsResponse = await fetchWithDeadline(
      `http://127.0.0.1:${devtoolsPort}/json/list`, {}, deadline, 'Chromium DevTools target list');
    const targets = await withDeadline(targetsResponse.json(), deadline, 'Chromium DevTools target list body');
    const target = targets.find(candidate => candidate.type === 'page');
    if (!target?.webSocketDebuggerUrl) throw new Error('Chromium did not expose a page target');
    session = new CdpSession(target.webSocketDebuggerUrl, (method, params) => {
      if (method === 'Runtime.exceptionThrown') {
        pageErrors.push(params.exceptionDetails?.exception?.description ?? params.exceptionDetails?.text ?? 'Unknown page error');
      } else if (method === 'Runtime.consoleAPICalled' && ['error', 'assert'].includes(params.type)) {
        consoleErrors.push(params.args.map(remoteValue).join(' '));
      } else if (method === 'Log.entryAdded' && params.entry?.level === 'error') {
        logErrors.push(params.entry);
      } else if (method === 'Network.loadingFailed') {
        failedRequests.push({ errorText: params.errorText, type: params.type, canceled: params.canceled ?? false });
      } else if (method === 'Network.responseReceived' && params.response?.status >= 400) {
        httpErrors.push({ requestId: params.requestId, url: params.response.url, status: params.response.status });
      }
    }, deadline);
    await session.connect();
    await Promise.all([
      session.send('Page.enable'),
      session.send('Runtime.enable'),
      session.send('Log.enable'),
      session.send('Network.enable'),
    ]);
    version = await session.send('Browser.getVersion');
    await session.send('Page.navigate', { url: baseUrl });
    state = await waitFor(async () => {
      const result = await session.send('Runtime.evaluate', {
        expression: `(() => {
          const summary = document.querySelector('.model-summary');
          const badge = document.querySelector('.badge');
          const text = summary?.textContent ?? '';
          const style = summary ? getComputedStyle(summary) : null;
          const visible = Boolean(summary && style && style.display !== 'none' && style.visibility !== 'hidden'
            && summary.getBoundingClientRect().width > 0 && summary.getBoundingClientRect().height > 0);
          return { title: document.title, text, badge: badge?.textContent ?? '', visible,
            instructionVisible: visible && text.includes(${JSON.stringify(DOWNLOAD_INSTRUCTION)}),
            windowsCommandVisible: visible && text.includes('.\\\\stockfly-server.exe fetch-models'),
            noTrainingVisible: visible && text.includes('No training is required.') };
        })()`,
        returnByValue: true,
      });
      return result.result?.value?.instructionVisible ? result.result.value : null;
    }, deadline, 'visible no-model download instruction', false);
    // Allow browser-generated errors queued with the final render to reach DevTools.
    await session.send('Runtime.evaluate', { expression: 'new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))', awaitPromise: true });
    const expectedHttpErrors = httpErrors.filter(item => isExpectedNoModelHttpError(item, baseUrl));
    const expectedRequestIds = new Set(expectedHttpErrors
      .map(item => item.requestId));
    const unexpectedLogErrors = logErrors.filter(entry => !isExpectedNoModelLog(entry, expectedRequestIds, baseUrl));
    const unexpectedHttpErrors = httpErrors.filter(item => !isExpectedNoModelHttpError(item, baseUrl));
    if (pageErrors.length || consoleErrors.length || unexpectedLogErrors.length || failedRequests.length || unexpectedHttpErrors.length) {
      throw new Error('Headless Chromium reported page, console, log, request, or unexpected HTTP errors');
    }
    if (state.title !== 'StockFly' || state.badge !== 'Unavailable' || !state.windowsCommandVisible || !state.noTrainingVisible) {
      throw new Error('No-model page did not expose the complete download-first state');
    }
    result = {
      executable: browserExecutable,
      product: version.product,
      userAgent: version.userAgent,
      headless: true,
      title: state.title,
      noModelBadge: state.badge,
      instruction: state.text,
      instructionVisible: state.instructionVisible,
      windowsCommandVisible: state.windowsCommandVisible,
      noTrainingVisible: state.noTrainingVisible,
      pageErrors,
      consoleErrors,
      browserLogErrors: unexpectedLogErrors.map(entry => ({ source: entry.source, text: entry.text })),
      expectedNoModelLogs: logErrors
        .filter(entry => isExpectedNoModelLog(entry, expectedRequestIds, baseUrl))
        .map(entry => ({ source: entry.source, text: entry.text, url: entry.url })),
      expectedNoModelHttpErrors: expectedHttpErrors.map(item => ({ url: item.url, status: item.status })),
      failedRequests,
      unexpectedHttpErrors,
    };
  } catch (error) {
    failure = error instanceof Error ? error : new Error(String(error));
  } finally {
    if (session) {
      try {
        await session.close();
      } catch (error) {
        failure ??= error instanceof Error ? error : new Error(String(error));
      }
    }
    if (browser) await stopProcess(browser, Date.now() + 2_000);
    if (browser?.exitCode && browser.exitCode !== 0) {
      const detail = browserStderr() || browserStdout();
      if (detail) process.stderr.write(`${detail}\n`);
    }
  }
  if (failure) {
    failure.browserEvidence = {
      executable: browserExecutable,
      ...(version ? { product: version.product, userAgent: version.userAgent } : {}),
      headless: true,
      ...(state ? { title: state.title, noModelBadge: state.badge, instruction: state.text } : {}),
      pageErrors,
      consoleErrors,
      browserLogErrors: logErrors.map(entry => ({ source: entry.source, text: entry.text, url: entry.url })),
      failedRequests,
      httpErrors,
    };
    throw failure;
  }
  return result;
}

async function stopProcess(child, deadline = Date.now() + 2_000) {
  if (!child || child.exitCode !== null || child.signalCode !== null || !child.pid) return;
  const exited = new Promise(resolve => child.once('exit', resolve));
  child.kill();
  await Promise.race([exited, delay(Math.max(1, deadline - Date.now()))]);
  if (child.exitCode === null) child.kill('SIGKILL');
}

export async function runSmoke(options) {
  const startedAt = new Date().toISOString();
  const evidence = {
    formatVersion: FORMAT_VERSION,
    smoke: 'stockfly-portable-runtime',
    startedAt,
    platform: process.platform,
    architecture: process.arch,
    source: {},
    pass: false,
  };
  let workRoot;
  let server;
  let serverStdout = () => '';
  let serverStderr = () => '';
  try {
    evidence.source = {
      gitCommit: process.env.GITHUB_SHA
        ?? (await run('git', ['rev-parse', 'HEAD'], { cwd: REPOSITORY_ROOT })).stdout,
      ...(process.env.GITHUB_RUN_ID ? { githubRunId: process.env.GITHUB_RUN_ID } : {}),
      ...(process.env.GITHUB_RUN_ATTEMPT ? { githubRunAttempt: process.env.GITHUB_RUN_ATTEMPT } : {}),
    };
    workRoot = await mkdtemp(path.join(tmpdir(), 'stockfly-portable-smoke-'));
    const archive = await resolveArchive(options.archive);
    evidence.archive = { name: path.basename(archive), sha256: await sha256(archive) };
    const bundle = await extractArchive(archive, path.join(workRoot, 'extracted'));
    const executableName = process.platform === 'win32' ? 'stockfly-server.exe' : 'stockfly-server';
    const executable = path.join(bundle, executableName);
    if (process.platform !== 'win32') await chmod(executable, 0o755);
    evidence.server = { executable: executableName, sha256: await sha256(executable) };
    const port = await availablePort();
    const baseUrl = `http://127.0.0.1:${port}`;
    const managedServer = spawnManaged(executable, ['--bind', `127.0.0.1:${port}`], {
      cwd: bundle,
    });
    server = managedServer.child;
    serverStdout = collectOutput(server.stdout);
    serverStderr = collectOutput(server.stderr);
    let serverExit;
    server.once('exit', code => { serverExit = code; });
    const runtimeDeadline = Date.now() + options.timeoutMs;
    await withDeadline(managedServer.started, runtimeDeadline, 'portable server process start', () => server.kill());
    await waitFor(async () => {
      if (managedServer.spawnError) throw managedServer.spawnError;
      if (serverExit !== undefined) throw new Error(`stockfly-server exited ${serverExit}: ${serverStderr() || serverStdout()}`);
      const response = await fetchWithDeadline(`${baseUrl}/health`, {}, runtimeDeadline, 'portable server health response');
      return response.ok;
    }, runtimeDeadline, 'portable server health');
    evidence.server.bind = `127.0.0.1:${port}`;
    evidence.routes = [];
    evidence.routes.push(await checkRoute(baseUrl, '/', 200, (body, response) => {
      if (!body.includes('<title>StockFly</title>')) throw new Error('/ did not serve the StockFly app');
      assertIsolationHeaders(response, '/');
    }, runtimeDeadline));
    evidence.routes.push(await checkRoute(baseUrl, '/favicon.svg', 200, (body, response) => {
      if (!body.includes('<svg')) throw new Error('/favicon.svg did not serve the tracked SVG icon');
      assertIsolationHeaders(response, '/favicon.svg');
    }, runtimeDeadline));
    evidence.routes.push(await checkRoute(baseUrl, '/health', 200, (body, response) => {
      if (JSON.parse(body).status !== 'ok') throw new Error('/health did not report ok');
      assertIsolationHeaders(response, '/health');
    }, runtimeDeadline));
    evidence.routes.push(await checkRoute(baseUrl, '/models', 200, (body, response) => {
      const models = JSON.parse(body);
      if (!Array.isArray(models) || models.length !== 0) throw new Error('/models did not report the expected empty no-model state');
      assertIsolationHeaders(response, '/models');
    }, runtimeDeadline));
    evidence.routes.push(await checkRoute(baseUrl, '/vendor/models/catalog.json', 404, (body, response) => {
      if (body !== 'not found') throw new Error('Missing no-model catalog did not return the expected response');
      assertIsolationHeaders(response, '/vendor/models/catalog.json');
    }, runtimeDeadline));
    evidence.noModelState = { localModels: [], catalogInstalled: false };
    evidence.browser = await browserSmoke(resolveBrowser(options.browser), baseUrl, workRoot, options.timeoutMs);
    evidence.pass = true;
    evidence.completedAt = new Date().toISOString();
    return evidence;
  } catch (error) {
    evidence.error = error instanceof Error ? error.message : String(error);
    if (error?.browserEvidence) evidence.browser = error.browserEvidence;
    evidence.serverLog = { stdout: serverStdout(), stderr: serverStderr() };
    evidence.completedAt = new Date().toISOString();
    throw Object.assign(error instanceof Error ? error : new Error(String(error)), { evidence });
  } finally {
    if (server) await stopProcess(server);
    if (workRoot) await rm(workRoot, { recursive: true, force: true });
  }
}

function fallbackEvidence(error) {
  return {
    formatVersion: FORMAT_VERSION,
    smoke: 'stockfly-portable-runtime',
    startedAt: new Date().toISOString(),
    platform: process.platform,
    architecture: process.arch,
    pass: false,
    error: error instanceof Error ? error.message : String(error),
    completedAt: new Date().toISOString(),
  };
}

function evidencePathFromArgs(argv) {
  const index = argv.indexOf('--evidence');
  return index >= 0 && argv[index + 1] && !argv[index + 1].startsWith('--') ? argv[index + 1] : undefined;
}

async function writeEvidenceAtomic(output, evidence) {
  const resolved = path.resolve(output);
  await mkdir(path.dirname(resolved), { recursive: true });
  const temporary = `${resolved}.${process.pid}.${Date.now()}.tmp`;
  try {
    await writeFile(temporary, `${JSON.stringify(evidence, null, 2)}\n`);
    await rename(temporary, resolved);
  } finally {
    await rm(temporary, { force: true });
  }
  return resolved;
}

async function main() {
  const argv = process.argv.slice(2);
  let options;
  try {
    options = parseArgs(argv);
  } catch (error) {
    const evidencePath = evidencePathFromArgs(argv);
    if (evidencePath) {
      try {
        const output = await writeEvidenceAtomic(evidencePath, fallbackEvidence(error));
        console.log(`SMOKE_EVIDENCE=${output}`);
      } catch (writeError) {
        console.error(`Could not write smoke evidence: ${writeError.message}`);
      }
    }
    console.error(error.message);
    process.exitCode = 1;
    return;
  }
  let evidence;
  try {
    evidence = await runSmoke(options);
  } catch (error) {
    evidence = error.evidence ?? fallbackEvidence(error);
    process.exitCode = 1;
  }
  const output = await writeEvidenceAtomic(options.evidence, evidence);
  console.log(`SMOKE_EVIDENCE=${output}`);
  if (!evidence.pass) console.error(`Portable runtime smoke failed: ${evidence.error}`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  await main();
}
