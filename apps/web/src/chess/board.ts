import type { Piece, Square } from 'chess.js';

export function displaySquares(board: (Piece | null)[][], orientation: 'w' | 'b') {
  return Array.from({ length: 64 }, (_, i) => {
    const row = Math.floor(i / 8);
    const column = i % 8;
    const rank = orientation === 'w' ? 7 - row : row;
    const file = orientation === 'w' ? column : 7 - column;
    const square = `${'abcdefgh'[file]}${rank + 1}` as Square;
    return { square, piece: board[7 - rank][file], light: (rank + file) % 2 === 1, row, column };
  });
}
export const pieceNames: Record<string, string> = { p: 'pawn', n: 'knight', b: 'bishop', r: 'rook', q: 'queen', k: 'king' };
