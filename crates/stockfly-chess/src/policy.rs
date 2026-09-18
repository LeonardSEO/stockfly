use std::str::FromStr;

use shakmaty::fen::Fen;
use shakmaty::uci::UciMove;
use shakmaty::{Chess, Position, Role};

use crate::output_map::{OutputMap, PROMOTION_ORDER};

#[derive(Debug)]
pub struct MoveDecision {
    pub uci: String,
    pub legal_scores: Vec<(String, f32)>,
    pub from_rates: [f32; 64],
    pub to_rates: [f32; 64],
    pub promotion_rates: [f32; 4],
}

#[derive(Debug)]
pub enum PolicyError {
    InvalidFen(String),
    NoLegalMoves,
}

impl std::fmt::Display for PolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyError::InvalidFen(s) => write!(f, "invalid FEN: {s}"),
            PolicyError::NoLegalMoves => write!(f, "no legal moves in this position"),
        }
    }
}
impl std::error::Error for PolicyError {}

pub type Result<T> = std::result::Result<T, PolicyError>;

fn group_rate(brain_rates: &[f32], group: &[u32]) -> f32 {
    let sum: f32 = group.iter().map(|&n| brain_rates.get(n as usize).copied().unwrap_or(0.0)).sum();
    sum / group.len().max(1) as f32
}

/// Score every legal move from the fixed neural from/to/promotion
/// populations and pick the highest-scoring one. No Stockfish evaluation
/// or chess-heuristic term is allowed here -- only real simulator output.
pub fn choose_move(fen: &str, brain_rates: &[f32], map: &OutputMap) -> Result<MoveDecision> {
    let setup = Fen::from_str(fen).map_err(|e| PolicyError::InvalidFen(e.to_string()))?;
    let position: Chess = setup
        .into_position(shakmaty::CastlingMode::Standard)
        .map_err(|e| PolicyError::InvalidFen(e.to_string()))?;

    let mut from_rates = [0.0f32; 64];
    let mut to_rates = [0.0f32; 64];
    let mut promotion_rates = [0.0f32; 4];
    for sq in 0..64 {
        from_rates[sq] = group_rate(brain_rates, &map.from_groups[sq]);
        to_rates[sq] = group_rate(brain_rates, &map.to_groups[sq]);
    }
    for (i, name) in PROMOTION_ORDER.iter().enumerate() {
        promotion_rates[i] = group_rate(brain_rates, map.promotion_group(name).unwrap_or(&[]));
    }

    let legal = position.legal_moves();
    if legal.is_empty() {
        return Err(PolicyError::NoLegalMoves);
    }

    let mut scored: Vec<(String, f32)> = legal
        .iter()
        .map(|m| {
            let uci = UciMove::from_standard(m).to_string();
            let from_idx = m.from().map(|s| s as usize);
            let to_idx = m.to() as usize;
            let promo_idx = m.promotion().map(|role| match role {
                Role::Queen => 0,
                Role::Rook => 1,
                Role::Bishop => 2,
                Role::Knight => 3,
                _ => usize::MAX,
            });

            let mut score = to_rates[to_idx];
            if let Some(fi) = from_idx {
                score += from_rates[fi];
            }
            if let Some(pi) = promo_idx {
                if pi != usize::MAX {
                    score += promotion_rates[pi];
                }
            }
            (uci, score)
        })
        .collect();

    // Stable, deterministic tie-break: sort by (-score, uci) so the highest
    // score wins and equal scores fall back to lexicographically smallest UCI.
    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });

    let best = scored.first().cloned().ok_or(PolicyError::NoLegalMoves)?;

    Ok(MoveDecision {
        uci: best.0,
        legal_scores: scored,
        from_rates,
        to_rates,
        promotion_rates,
    })
}
