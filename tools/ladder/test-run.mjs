import test from 'node:test';
import assert from 'node:assert/strict';
import { Chess } from 'chess.js';
import { LineProcess, opening, terminal, playGame, sameIdentity, parseBudgets } from './run.mjs';
const config = { opening_seed: 20260918, opening_plies: 8, max_plies: 0 };
test('explicit low-node budgets preserve defaults and reject invalid conditions', () => {
  assert.deepEqual(parseBudgets('1,5,10,25,50', [50, 100]), [1, 5, 10, 25, 50]);
  assert.deepEqual(parseBudgets(undefined, [50, 100]), [50, 100]);
  for (const value of ['', '0', '-1', '1.5', '1,1', '1,', 'NaN', 'Infinity', '9007199254740992']) {
    assert.throws(() => parseBudgets(value, [50]), /invalid budgets/);
  }
});
test('distinct reproducible paired openings', () => {
  const positions = Array.from({length:10},(_,p)=>opening(config,p));
  assert.equal(new Set(positions.map(p=>p.fen)).size,10);
  assert.deepEqual(positions,Array.from({length:10},(_,p)=>opening(config,p)));
});
test('checkmate uses losing side to move and draws have reasons', () => {
  const chess = new Chess(); for (const move of ['f3','e5','g4','Qh4#']) chess.move(move);
  assert.deepEqual(terminal(chess,'w'),{termination:'checkmate',score:0});
  assert.deepEqual(terminal(chess,'b'),{termination:'checkmate',score:1});
  assert.equal(terminal(new Chess('7k/5Q2/6K1/8/8/8/8/8 b - - 0 1'),'w').termination,'stalemate');
  assert.equal(terminal(new Chess('7k/8/6K1/8/8/8/8/8 b - - 0 1'),'w').termination,'insufficient-material');
});
test('cap and failures never become draws; model gets only FEN', async () => {
  const cap = await playGame({fly:{},opponent:{newGame:async()=>{}},config,nodes:50,pair:0,color:'w'});
  assert.equal(cap.termination,'ply-cap'); assert.equal(cap.score,null);
  const error = await playGame({fly:{move:async(...args)=>{assert.equal(args.length,1);assert.equal(typeof args[0],'string');throw new Error('bad model');}},opponent:{newGame:async()=>{}},config:{...config,max_plies:1},nodes:50,pair:0,color:'w'});
  assert.equal(error.termination,'model-error');assert.equal(error.score,null);
});
test('resume rejects stale identity',()=>{
  sameIdentity({hash:'a'},{hash:'a'});
  assert.throws(()=>sameIdentity({hash:'a'},{hash:'b'}));
});
test('process timeout/exit reject and info is not accumulated', async()=>{
  const proc = new LineProcess(process.execPath,['-e',"console.log('info pv secret'); setTimeout(()=>console.log('ready'),20); setInterval(()=>{},1000)"],1000);
  assert.equal(await proc.wait(x=>x==='ready'),'ready');
  assert.equal(proc.pending,null);assert.equal(proc.pendingLines,undefined);
  proc.close();
  const timeout = new LineProcess(process.execPath,['-e','setInterval(()=>{},1000)'],50);
  await assert.rejects(timeout.wait(()=>false),/timeout/);timeout.close();
  const exit = new LineProcess(process.execPath,['-e','process.exit(2)'],1000);
  await assert.rejects(exit.wait(()=>false),/process exit/);exit.close();
});
