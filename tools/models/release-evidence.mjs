import { readFile } from 'node:fs/promises';
import path from 'node:path';

// Reports remain the source of measured outcomes. Publication only checks their
// packaged identities and keeps their declared scope; it does not rerun science.
export async function collectReleaseEvidence(input, models, files) {
  const fileAt = name => {
    const file = files.find(item => item.path === name);
    if (!file) throw Error(`Missing release evidence or input: ${name}`);
    return file;
  };
  const verify = (name, expected) => {
    const file = fileAt(name);
    if (file.sha256 !== expected) throw Error(`Evidence hash mismatch: ${name}`);
    return { path: file.path, sha256: file.sha256 };
  };
  const summary = async name => {
    const file = fileAt(`data/release-evidence/2026-09-19-${name}.json`);
    return [JSON.parse(await readFile(path.join(input, file.path), 'utf8')), { path: file.path, sha256: file.sha256 }];
  };
  const [causal, causalSummary] = await summary('standard-causal-audit');
  const [ladder, ladderSummary] = await summary('standard-playing-strength');
  const [parity, paritySummary] = await summary('fullgraph-parity');
  const [training, trainingSummary] = await summary('standard-training-sweep');
  const [windows, windowsSummary] = await summary('windows-runtime');
  if (windows.pass !== true || windows.platform !== 'win32' || windows.noModelState.catalogInstalled !== false ||
      !windows.browser.instructionVisible || !windows.browser.windowsCommandVisible || !windows.browser.noTrainingVisible ||
      ['pageErrors', 'consoleErrors', 'browserLogErrors', 'failedRequests', 'unexpectedHttpErrors'].some(key => windows.browser[key].length)) throw Error('Expected passing Windows no-model runtime/browser evidence');
  const bind = (identity, model) => {
    if (identity.model_sha256 !== model.checkpointSha256) throw Error(`Evidence checkpoint mismatch: ${model.id}`);
    for (const [name, sha256] of Object.entries(identity.graph_files)) verify(`data/compiled/malecns-v1/${name}`, sha256);
    if (identity.graph_files['neurons.bin'] !== model.graphNeuronsSha256) throw Error(`Evidence graph mismatch: ${model.id}`);
    verify('data/chess-maps/sensory-map.json', identity.sensory_map_sha256);
    verify('data/chess-maps/output-map.json', identity.output_map_sha256);
    if (identity.sensory_map_sha256 !== model.sensoryMapSha256 || identity.output_map_sha256 !== model.outputMapSha256) throw Error(`Evidence maps mismatch: ${model.id}`);
  };
  if (causal.status !== 'FAIL' || causal.reports.length !== 2) throw Error('Expected the two failed canonical causal audits');
  const selections = [];
  for (const model of models) {
    const audit = causal.reports.find(report => report.model_kind === model.id);
    const match = ladder.models[model.id === 'bio-full' ? 'bio' : 'max'];
    const numerical = parity.final.models.find(report => report.model_kind === model.id);
    const candidate = training.candidates.find(item => item.sha256 === model.checkpointSha256);
    if (!audit || !match || !numerical || !candidate || candidate.kind !== model.id || candidate.trialsRun !== model.trialsRun || model.trainingPreset !== 'standard') throw Error(`Missing matching Standard evidence: ${model.id}`);
    if (audit.gate.status !== 'FAIL' || audit.calibration.settle_steps !== 16 || match.identity.model.calibration.settle_steps !== 16 || parity.final.configuration.settle_steps !== 16) throw Error(`Unexpected gate or settling calibration: ${model.id}`);
    bind(audit.inputs, model);
    bind(match.identity.model.hashes, model);
    bind(numerical.identity, model);
    if (candidate.seed !== match.selection_seed || candidate.seed !== numerical.seed) throw Error(`Evidence selection seed mismatch: ${model.id}`);
    verify(`data/release-evidence/causal/${path.basename(audit.raw_report)}`, audit.raw_report_sha256);
    verify(`data/release-evidence/ladder/${path.basename(match.raw_report)}`, match.raw_report_sha256);
    selections.push({ kind: model.id, selectionSeed: candidate.seed, causalAuditStatus: 'failed' });
  }
  for (const report of causal.superseded_evidence.reports) verify(`data/release-evidence/causal-superseded/${path.basename(report.raw_report)}`, report.raw_report_sha256);
  verify('data/release-evidence/causal/superseded-summary.json', causal.superseded_evidence.tracked_summary_sha256);
  for (const phase of ['baseline', 'diagnostic', 'final']) verify(`data/release-evidence/parity/${path.basename(parity[phase].raw_report.path)}`, parity[phase].raw_report.sha256);
  const rows = Object.values(ladder.models).flatMap(model => Object.values(model.summary));
  const cases = parity.final.models.flatMap(model => model.positions);
  return {
    selections,
    evidence: {
      training: { summary: trainingSummary, selection: 'Highest observed training-stream top-1; not held-out accuracy or scientific promotion.' },
      causalAudit: { status: 'failed', summary: causalSummary, settleSteps: 16, evaluatorSha256: causal.provenance.binary_sha256, scope: causal.evidence_status },
      playingStrength: { summary: ladderSummary, budgets: ladder.budgets, totals: ladder.totals, finitePointElo: rows.some(row => row.local_elo_point !== null), scope: 'Per exact Stockfish 19 Lite node budget; no absolute or human Elo; rows cannot be pooled.' },
      nativeParity: { status: parity.status === 'PASS' && cases.every(item => item.pass) ? 'passed' : 'failed', summary: paritySummary, cases: cases.length, scope: 'Fixed native Apple M4 Metal suite only; not browser WebGPU, Windows DX12 or universal FEN coverage.' },
      windowsRuntime: { status: 'passed', summary: windowsSummary, sourceCommit: windows.source.gitCommit, archiveSha256: windows.archive.sha256, scope: 'Download-first no-model runtime/browser smoke only, on the identified CI archive; no Windows model inference or DX12 parity claim.' },
    },
  };
}
