use std::collections::HashMap;
use std::str::FromStr;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use shakmaty::fen::Fen;
use shakmaty::{Board, CastlingSide, Chess, Color, EnPassantMode, Piece, Position, Role, Square};
use stockfly_sim::Stimulus;

#[derive(Debug, Deserialize)]
pub struct SensoryMap {
    pub format_version: u32,
    pub seed: u64,
    pub neurons_per_square: usize,
    pub board_populations: Vec<Vec<u32>>,
    pub piece_embeddings: HashMap<String, Vec<f32>>,
    pub context: ContextMap,
    #[serde(default)]
    canonical_json: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ContextMap {
    pub side_to_move: u32,
    pub castling_rights: Vec<u32>,
    pub en_passant_file: Vec<u32>,
}

#[derive(Debug)]
pub enum SensoryError {
    InvalidFen(String),
    Json(serde_json::Error),
    BadMap(String),
}

impl std::fmt::Display for SensoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SensoryError::InvalidFen(s) => write!(f, "invalid FEN: {s}"),
            SensoryError::Json(e) => write!(f, "sensory map parse error: {e}"),
            SensoryError::BadMap(s) => write!(f, "invalid sensory map: {s}"),
        }
    }
}
impl std::error::Error for SensoryError {}
impl From<serde_json::Error> for SensoryError {
    fn from(e: serde_json::Error) -> Self {
        SensoryError::Json(e)
    }
}

pub type Result<T> = std::result::Result<T, SensoryError>;

fn piece_char(piece: Piece) -> char {
    let base = match piece.role {
        Role::Pawn => 'p',
        Role::Knight => 'n',
        Role::Bishop => 'b',
        Role::Rook => 'r',
        Role::Queen => 'q',
        Role::King => 'k',
    };
    if piece.color == Color::White {
        base.to_ascii_uppercase()
    } else {
        base
    }
}

impl SensoryMap {
    pub fn load_str(json: &str) -> Result<Self> {
        let mut map: SensoryMap = serde_json::from_str(json)?;
        if map.board_populations.len() != 64 {
            return Err(SensoryError::BadMap(format!(
                "expected 64 board squares, got {}",
                map.board_populations.len()
            )));
        }
        for (i, pop) in map.board_populations.iter().enumerate() {
            if pop.len() != map.neurons_per_square {
                return Err(SensoryError::BadMap(format!(
                    "square {i} has {} neurons, expected {}",
                    pop.len(),
                    map.neurons_per_square
                )));
            }
        }
        if map.context.castling_rights.len() != 4 {
            return Err(SensoryError::BadMap("expected 4 castling-right neurons".into()));
        }
        if map.context.en_passant_file.len() != 8 {
            return Err(SensoryError::BadMap("expected 8 en-passant-file neurons".into()));
        }
        // Canonical hash is computed over the raw JSON bytes as loaded, so
        // any change to the committed resource changes every downstream
        // checkpoint/trace hash that references it.
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        map.canonical_json = Some(format!("{:x}", hasher.finalize()));
        Ok(map)
    }

    pub fn sha256(&self) -> &str {
        self.canonical_json.as_deref().unwrap_or("")
    }
}

/// Encode a FEN position into a dense per-neuron stimulus vector. Same FEN
/// + map always produces byte-identical output. Only board placement, side
/// to move, castling rights, and en-passant file enter the stimulus --
/// never the legal-move list.
pub fn encode_position(fen: &str, map: &SensoryMap, neuron_count: usize) -> Result<Stimulus> {
    let setup = Fen::from_str(fen).map_err(|e| SensoryError::InvalidFen(e.to_string()))?;
    let position: Chess = setup
        .into_position(shakmaty::CastlingMode::Standard)
        .map_err(|e| SensoryError::InvalidFen(e.to_string()))?;

    let mut stimulus = Stimulus::zeroed(neuron_count);
    encode_board(position.board(), map, &mut stimulus)?;
    encode_context(&position, map, &mut stimulus)?;
    Ok(stimulus)
}

fn encode_board(board: &Board, map: &SensoryMap, stimulus: &mut Stimulus) -> Result<()> {
    for square in Square::ALL {
        let sq_index = square as usize; // shakmaty::Square is a1=0 .. h8=63
        let dense_neurons = &map.board_populations[sq_index];
        if let Some(piece) = board.piece_at(square) {
            let ch = piece_char(piece).to_string();
            let embedding = map
                .piece_embeddings
                .get(&ch)
                .ok_or_else(|| SensoryError::BadMap(format!("no embedding for piece '{ch}'")))?;
            for (neuron, value) in dense_neurons.iter().zip(embedding.iter()) {
                set_stimulus(stimulus, *neuron, *value)?;
            }
        }
        // Empty squares leave their neurons at 0.0 (Stimulus::zeroed default).
    }
    Ok(())
}

fn encode_context(position: &Chess, map: &SensoryMap, stimulus: &mut Stimulus) -> Result<()> {
    let side_value = if position.turn() == Color::Black { 1.0 } else { 0.0 };
    set_stimulus(stimulus, map.context.side_to_move, side_value)?;

    let castles = position.castles();
    // Order: [K, Q, k, q] per generate_neural_maps.py's context_bodies[1:5].
    let rights = [
        castles.has(Color::White, CastlingSide::KingSide),
        castles.has(Color::White, CastlingSide::QueenSide),
        castles.has(Color::Black, CastlingSide::KingSide),
        castles.has(Color::Black, CastlingSide::QueenSide),
    ];
    for (neuron, has_right) in map.context.castling_rights.iter().zip(rights.iter()) {
        set_stimulus(stimulus, *neuron, if *has_right { 1.0 } else { 0.0 })?;
    }

    if let Some(ep_square) = position.ep_square(EnPassantMode::Legal) {
        let file = ep_square.file() as usize; // 0 = 'a' .. 7 = 'h'
        set_stimulus(stimulus, map.context.en_passant_file[file], 1.0)?;
    }

    Ok(())
}

fn set_stimulus(stimulus: &mut Stimulus, dense_index: u32, value: f32) -> Result<()> {
    let idx = dense_index as usize;
    let slot = stimulus
        .values
        .get_mut(idx)
        .ok_or_else(|| SensoryError::BadMap(format!("dense index {idx} out of range for this graph")))?;
    *slot += value;
    Ok(())
}
