//! Local two-player Elo differences; opening/color pairs are the sampling unit.
use crate::audit::shuffle::SeededRng;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{self, Read},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Deserialize)]
pub struct Game {
    pub pair: u32,
    pub color: String,
    pub termination: String,
    pub score: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub planned_games: usize,
    pub terminal_games: usize,
    pub eligible_games: usize,
    pub excluded_pairs: usize,
    pub pairs: usize,
    pub wins: usize,
    pub draws: usize,
    pub losses: usize,
    pub terminations: BTreeMap<String, usize>,
    pub score: Option<f64>,
    pub score_ci: Option<[f64; 2]>,
    pub local_elo_difference: Option<String>,
    pub local_elo_ci: Option<[String; 2]>,
    pub method: String,
    pub reference: &'static str,
}

pub fn elo(score: f64) -> String {
    if score == 0.0 {
        "-infinity".into()
    } else if score == 1.0 {
        "+infinity".into()
    } else {
        format!("{:.0}", 400.0 * (score / (1.0 - score)).log10())
    }
}

pub fn summarize(games: &[Game], replicates: usize, seed: u64) -> Result<Summary> {
    if replicates < 10000 {
        return Err("at least 10000 bootstrap replicates required".into());
    }
    let mut groups: BTreeMap<u32, Vec<&Game>> = BTreeMap::new();
    let mut summary = Summary {
        planned_games: games.len(),
        terminal_games: 0,
        eligible_games: 0,
        excluded_pairs: 0,
        pairs: 0,
        wins: 0,
        draws: 0,
        losses: 0,
        terminations: BTreeMap::new(),
        score: None,
        score_ci: None,
        local_elo_difference: None,
        local_elo_ci: None,
        method: "no complete eligible pairs".into(),
        reference:
            "difference against this exact Stockfish build/node budget; no external Elo anchor",
    };
    for game in games {
        if !["w", "b"].contains(&game.color.as_str()) {
            return Err("invalid color".into());
        }
        let terminal = [
            "checkmate",
            "stalemate",
            "insufficient-material",
            "threefold-repetition",
            "fifty-move",
        ]
        .contains(&game.termination.as_str());
        if terminal != game.score.is_some()
            || game.score.is_some_and(|s| ![0.0, 0.5, 1.0].contains(&s))
        {
            return Err("nonterminal result scored, or invalid terminal score".into());
        }
        *summary
            .terminations
            .entry(game.termination.clone())
            .or_default() += 1;
        if let Some(score) = game.score {
            summary.terminal_games += 1;
            if score == 1.0 {
                summary.wins += 1;
            } else if score == 0.5 {
                summary.draws += 1;
            } else {
                summary.losses += 1;
            }
        }
        groups.entry(game.pair).or_default().push(game);
    }
    let mut clusters = Vec::new();
    for group in groups.values() {
        if group.len() > 2 || (group.len() == 2 && group[0].color == group[1].color) {
            return Err("duplicate or color-imbalanced opening pair".into());
        }
        if group.len() == 2 && group.iter().all(|g| g.score.is_some()) {
            clusters.push((group[0].score.unwrap() + group[1].score.unwrap()) / 2.0);
        } else {
            summary.excluded_pairs += 1;
        }
    }
    let k = clusters.len();
    if k == 0 {
        return Ok(summary);
    }
    summary.pairs = k;
    summary.eligible_games = 2 * k;
    let score = clusters.iter().sum::<f64>() / k as f64;
    let ci = if clusters.iter().all(|&x| x == clusters[0]) {
        let extreme = score == 0.0 || score == 1.0;
        let epsilon = ((if extreme { 20_f64 } else { 40_f64 }).ln() / (2.0 * k as f64)).sqrt();
        summary.method = if extreme {
            "one-sided 95% Hoeffding bound"
        } else {
            "two-sided 95% Hoeffding interval (constant clusters)"
        }
        .into();
        [(score - epsilon).max(0.0), (score + epsilon).min(1.0)]
    } else {
        let mut rng = SeededRng::new(seed);
        let mut samples = Vec::with_capacity(replicates);
        for _ in 0..replicates {
            samples.push(
                (0..k)
                    .map(|_| clusters[(rng.next_u64() % k as u64) as usize])
                    .sum::<f64>()
                    / k as f64,
            );
        }
        samples.sort_by(f64::total_cmp);
        summary.method =
            format!("95% percentile bootstrap of whole opening pairs ({replicates} replicates)");
        [
            samples[(replicates as f64 * 0.025).floor() as usize],
            samples[(replicates as f64 * 0.975).ceil() as usize - 1],
        ]
    };
    summary.score = Some(score);
    summary.score_ci = Some(ci);
    // A boundary sample has no finite maximum-likelihood point estimate.
    summary.local_elo_difference = if score == 0.0 {
        Some(format!("unbounded below; 95% upper bound {}", elo(ci[1])))
    } else if score == 1.0 {
        Some(format!("unbounded above; 95% lower bound {}", elo(ci[0])))
    } else {
        Some(elo(score))
    };
    summary.local_elo_ci = Some([elo(ci[0]), elo(ci[1])]);
    Ok(summary)
}

pub fn run_cli() -> Result<String> {
    #[derive(Deserialize)]
    struct Request {
        games: Vec<Game>,
        replicates: usize,
        seed: u64,
    }
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let request: Request = serde_json::from_str(&input)?;
    Ok(serde_json::to_string_pretty(&summarize(
        &request.games,
        request.replicates,
        request.seed,
    )?)?)
}
