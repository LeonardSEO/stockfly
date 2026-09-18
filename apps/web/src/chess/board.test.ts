import test from 'node:test';
import assert from 'node:assert/strict';
import { Chess } from 'chess.js';
import { displaySquares } from './board.ts';
import { Game } from './game.ts';
import { InferenceLifecycle } from '../engine/lifecycle.ts';

test('board coordinates match real pieces, for white and black orientations', () => {
  const chess = new Chess();
  for (const orientation of ['w', 'b'] as const) {
    const squares = displaySquares(chess.board(), orientation);
    assert.equal(squares.length, 64);
    assert.equal(new Set(squares.map(cell => cell.square)).size, 64);
    for (const cell of squares) {
      const piece = chess.get(cell.square);
      assert.deepEqual(cell.piece, piece ? { ...piece, square: cell.square } : null);
    }
    assert.equal(squares.find(cell => cell.square === 'a8')!.piece!.color, 'b');
    assert.equal(squares.find(cell => cell.square === 'a1')!.piece!.color, 'w');
    assert.equal(squares.find(cell => cell.square === 'a1')!.light, false);
    assert.equal(squares[0].square, orientation === 'w' ? 'a8' : 'h1');
  }
});
test('a legal human move changes the correct physical square', () => {
  const game = new Game();
  assert.equal(game.applyMove('e2', 'e4'), 'e4');
  const cells = displaySquares(game.board(), 'w');
  assert.equal(cells.find(cell => cell.square === 'e2')!.piece, null);
  assert.equal(cells.find(cell => cell.square === 'e4')!.piece!.type, 'p');
});
test('new game invalidates old frames, decisions, and errors; new trace rejects previous trace', () => {
  const life = new InferenceLifecycle();
  const old = life.begin();
  assert.equal(life.accepts(old), true);
  life.reset();
  const current = life.begin();
  for (const type of ['frame', 'decision', 'error']) assert.equal(life.accepts({ ...old, type } as typeof old), false);
  assert.equal(life.accepts(current), true);
  const next = life.begin();
  assert.equal(life.accepts(current), false);
  assert.equal(life.accepts(next), true);
  life.finish(); assert.equal(life.accepts(next), false);
});
