#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { createReadStream, existsSync } from 'node:fs';
import { chmod, mkdir, mkdtemp, readFile, readdir, rm, stat, writeFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawn } from 'node:child_process';

const FORMAT_VERSION = 1;
const DOWNLOAD_INSTRUCTION = 'Download pretrained model:';
const REPOSITORY_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

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

function delay(milliseconds) {
  return new Promise(resolve => setTimeout(resolve, milliseconds));
}

async function waitFor(predicate, timeoutMs, description) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const value = await predicate();
      if (value) return value;
    } catch (error) {
      lastError = error;
    }
    await delay(100);
  }
  throw new Error(`Timed out waiting for ${description}${lastError ? `: ${lastError.message}` : ''}`);
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

export async function checkRoute(baseUrl, route, expectedStatus, validate) {
  const response = await fetch(`${baseUrl}${route}`, { redirect: 'manual' });
  const body = await response.text();
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

class CdpSession {
  constructor(url, onEvent) {
    this.url = url;
    this.onEvent = onEvent;
    this.nextId = 1;
    this.pending = new Map();
  }

  async connect() {
    this.socket = new WebSocket(this.url);
    this.socket.onmessage = event => {
      const message = JSON.parse(event.data);
      if (message.id) {
        const pending = this.pending.get(message.id);
        if (!pending) return;
        this.pending.delete(message.id);
        if (message.error) pending.reject(new Error(message.error.message));
        else pending.resolve(message.result);
      } else {
        this.onEvent(message.method, message.params ?? {});
      }
    };
    await new Promise((resolve, reject) => {
      this.socket.onopen = resolve;
      this.socket.onerror = () => reject(new Error(`Could not connect to Chromium DevTools at ${this.url}`));
    });
  }

  send(method, params = {}) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.socket.send(JSON.stringify({ id, method, params }));
    });
  }

  close() {
    this.socket?.close();
  }
}

function remoteValue(argument) {
  if ('value' in argument) return String(argument.value);
  return argument.description ?? argument.type ?? '';
}

export function isExpectedNoModelLog(entry, catalogRequestIds = new Set()) {
  return entry?.source === 'network'
    && entry?.level === 'error'
    && /404/.test(entry.text ?? '')
    && (/\/vendor\/models\/catalog\.json(?:$|[?#])/.test(entry.url ?? '')
      || catalogRequestIds.has(entry.networkRequestId));
}

async function browserSmoke(browserExecutable, baseUrl, workRoot, timeoutMs) {
  const profile = path.join(workRoot, 'chromium-profile');
  await mkdir(profile);
  const browser = spawn(browserExecutable, [
    '--headless=new',
    '--remote-debugging-address=127.0.0.1',
    '--remote-debugging-port=0',
    `--user-data-dir=${profile}`,
    '--no-first-run',
    '--no-default-browser-check',
    '--disable-background-networking',
    '--disable-component-update',
    'about:blank',
  ], { stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true });
  const browserStdout = collectOutput(browser.stdout);
  const browserStderr = collectOutput(browser.stderr);
  const activePortFile = path.join(profile, 'DevToolsActivePort');
  let session;
  const pageErrors = [];
  const consoleErrors = [];
  const logErrors = [];
  const failedRequests = [];
  const httpErrors = [];
  let version;
  let state;
  try {
    const devtoolsPort = await waitFor(async () => {
      const contents = await readFile(activePortFile, 'utf8');
      return Number(contents.split(/\r?\n/, 1)[0]) || null;
    }, timeoutMs, 'Chromium DevTools endpoint');
    const targets = await (await fetch(`http://127.0.0.1:${devtoolsPort}/json/list`)).json();
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
    });
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
    }, timeoutMs, 'visible no-model download instruction');
    // Allow browser-generated errors queued with the final render to reach DevTools.
    await session.send('Runtime.evaluate', { expression: 'new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))', awaitPromise: true });
    const catalogRequestIds = new Set(httpErrors
      .filter(item => item.status === 404 && item.url === `${baseUrl}/vendor/models/catalog.json`)
      .map(item => item.requestId));
    const unexpectedLogErrors = logErrors.filter(entry => !isExpectedNoModelLog(entry, catalogRequestIds));
    const unexpectedHttpErrors = httpErrors.filter(item => !(item.status === 404 && item.url === `${baseUrl}/vendor/models/catalog.json`));
    if (pageErrors.length || consoleErrors.length || unexpectedLogErrors.length || failedRequests.length || unexpectedHttpErrors.length) {
      throw new Error('Headless Chromium reported page, console, log, request, or unexpected HTTP errors');
    }
    if (state.title !== 'StockFly' || state.badge !== 'Unavailable' || !state.windowsCommandVisible || !state.noTrainingVisible) {
      throw new Error('No-model page did not expose the complete download-first state');
    }
    return {
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
        .filter(entry => isExpectedNoModelLog(entry, catalogRequestIds))
        .map(entry => ({ source: entry.source, text: entry.text, url: entry.url })),
      failedRequests,
      unexpectedHttpErrors,
    };
  } catch (error) {
    const failure = error instanceof Error ? error : new Error(String(error));
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
  } finally {
    session?.close();
    if (!browser.killed) browser.kill();
    await Promise.race([new Promise(resolve => browser.once('exit', resolve)), delay(2_000)]);
    if (browser.exitCode === null) browser.kill('SIGKILL');
    if (browser.exitCode && browser.exitCode !== 0) {
      const detail = browserStderr() || browserStdout();
      if (detail) process.stderr.write(`${detail}\n`);
    }
  }
}

async function stopProcess(child) {
  if (child.exitCode !== null) return;
  child.kill();
  await Promise.race([new Promise(resolve => child.once('exit', resolve)), delay(2_000)]);
  if (child.exitCode === null) child.kill('SIGKILL');
}

export async function runSmoke(options) {
  const startedAt = new Date().toISOString();
  const gitCommit = process.env.GITHUB_SHA
    ?? (await run('git', ['rev-parse', 'HEAD'], { cwd: REPOSITORY_ROOT })).stdout;
  const evidence = {
    formatVersion: FORMAT_VERSION,
    smoke: 'stockfly-portable-runtime',
    startedAt,
    platform: process.platform,
    architecture: process.arch,
    source: {
      gitCommit,
      ...(process.env.GITHUB_RUN_ID ? { githubRunId: process.env.GITHUB_RUN_ID } : {}),
      ...(process.env.GITHUB_RUN_ATTEMPT ? { githubRunAttempt: process.env.GITHUB_RUN_ATTEMPT } : {}),
    },
    pass: false,
  };
  const workRoot = await mkdtemp(path.join(tmpdir(), 'stockfly-portable-smoke-'));
  let server;
  let serverStdout = () => '';
  let serverStderr = () => '';
  try {
    const archive = await resolveArchive(options.archive);
    evidence.archive = { name: path.basename(archive), sha256: await sha256(archive) };
    const bundle = await extractArchive(archive, path.join(workRoot, 'extracted'));
    const executableName = process.platform === 'win32' ? 'stockfly-server.exe' : 'stockfly-server';
    const executable = path.join(bundle, executableName);
    if (process.platform !== 'win32') await chmod(executable, 0o755);
    evidence.server = { executable: executableName, sha256: await sha256(executable) };
    const port = await availablePort();
    const baseUrl = `http://127.0.0.1:${port}`;
    server = spawn(executable, ['--bind', `127.0.0.1:${port}`], {
      cwd: bundle,
      stdio: ['ignore', 'pipe', 'pipe'],
      windowsHide: true,
    });
    serverStdout = collectOutput(server.stdout);
    serverStderr = collectOutput(server.stderr);
    let serverExit;
    server.once('exit', code => { serverExit = code; });
    await waitFor(async () => {
      if (serverExit !== undefined) throw new Error(`stockfly-server exited ${serverExit}: ${serverStderr() || serverStdout()}`);
      const response = await fetch(`${baseUrl}/health`);
      return response.ok;
    }, options.timeoutMs, 'portable server health');
    evidence.server.bind = `127.0.0.1:${port}`;
    evidence.routes = [];
    evidence.routes.push(await checkRoute(baseUrl, '/', 200, (body, response) => {
      if (!body.includes('<title>StockFly</title>')) throw new Error('/ did not serve the StockFly app');
      assertIsolationHeaders(response, '/');
    }));
    evidence.routes.push(await checkRoute(baseUrl, '/health', 200, (body, response) => {
      if (JSON.parse(body).status !== 'ok') throw new Error('/health did not report ok');
      assertIsolationHeaders(response, '/health');
    }));
    evidence.routes.push(await checkRoute(baseUrl, '/models', 200, (body, response) => {
      const models = JSON.parse(body);
      if (!Array.isArray(models) || models.length !== 0) throw new Error('/models did not report the expected empty no-model state');
      assertIsolationHeaders(response, '/models');
    }));
    evidence.routes.push(await checkRoute(baseUrl, '/vendor/models/catalog.json', 404, (body, response) => {
      if (body !== 'not found') throw new Error('Missing no-model catalog did not return the expected response');
      assertIsolationHeaders(response, '/vendor/models/catalog.json');
    }));
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
    await rm(workRoot, { recursive: true, force: true });
  }
}

async function main() {
  let options;
  try {
    options = parseArgs(process.argv.slice(2));
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
    return;
  }
  let evidence;
  try {
    evidence = await runSmoke(options);
  } catch (error) {
    evidence = error.evidence ?? {
      formatVersion: FORMAT_VERSION,
      smoke: 'stockfly-portable-runtime',
      pass: false,
      error: error.message,
      completedAt: new Date().toISOString(),
    };
    process.exitCode = 1;
  }
  const output = path.resolve(options.evidence);
  await mkdir(path.dirname(output), { recursive: true });
  await writeFile(output, `${JSON.stringify(evidence, null, 2)}\n`);
  console.log(`SMOKE_EVIDENCE=${output}`);
  if (!evidence.pass) console.error(`Portable runtime smoke failed: ${evidence.error}`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  await main();
}
