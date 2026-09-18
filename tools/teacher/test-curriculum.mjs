import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));

test("generator emits examples across every configured stage", async () => {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "stockfly-curriculum-"));
  const configPath = path.join(tmpDir, "curriculum-config.json");
  const outPath = path.join(tmpDir, "tiny.jsonl");

  // A tiny, fast preset: a handful of examples per stage so the test runs
  // in a few seconds instead of minutes of real engine search.
  const config = {
    stages: {
      legal: { share: 1 / 6, nodes: 50 },
      capture: { share: 1 / 6, nodes: 50 },
      "check-evasion": { share: 1 / 6, nodes: 50 },
      mate1: { share: 1 / 6, nodes: 50 },
      endgame: { share: 1 / 6, nodes: 50 },
      mixed: { share: 1 / 6, nodes: 50 },
    },
    presets: {
      tiny: { total_examples: 12, wall_clock_seconds: 60 },
    },
  };
  fs.writeFileSync(configPath, JSON.stringify(config));

  const scriptPath = path.join(HERE, "generate-curriculum.mjs");

  execFileSync(
    "node",
    [scriptPath, "--preset", "tiny", "--out", outPath, "--seed", "7", "--config", configPath],
    { cwd: HERE, stdio: "inherit" }
  );

  const lines = fs
    .readFileSync(outPath, "utf8")
    .trim()
    .split("\n")
    .filter(Boolean)
    .map((l) => JSON.parse(l));

  assert.ok(lines.length > 0, "should have written at least one example");
  const stagesSeen = new Set(lines.map((l) => l.stage));
  for (const stage of ["legal", "capture", "check-evasion", "mate1", "endgame", "mixed"]) {
    assert.ok(stagesSeen.has(stage), `expected at least one example for stage '${stage}', got stages: ${[...stagesSeen]}`);
  }
  for (const row of lines) {
    assert.match(row.fen, / /);
    assert.match(row.bestmove, /^[a-h][1-8][a-h][1-8][qrbn]?$/);
  }

  fs.rmSync(tmpDir, { recursive: true, force: true });
});
