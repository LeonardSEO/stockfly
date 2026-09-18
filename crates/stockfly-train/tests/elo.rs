use stockfly_train::{
    elo::{elo, summarize, Game},
    ladder_infer::Request,
};
fn game(pair: u32, color: &str, score: Option<f64>) -> Game {
    Game {
        pair,
        color: color.into(),
        score,
        termination: if score.is_some() {
            "checkmate"
        } else {
            "model-timeout"
        }
        .into(),
    }
}
#[test]
fn paired_score_and_transform() {
    let games = [
        game(0, "w", Some(1.0)),
        game(0, "b", Some(0.5)),
        game(1, "w", Some(0.0)),
        game(1, "b", Some(0.0)),
    ];
    let result = summarize(&games, 10000, 42).unwrap();
    assert_eq!(result.score, Some(0.375));
    assert_eq!(result.local_elo_difference.as_deref(), Some("-89"));
    // Whole pairs have means 0.75/0; resampling games would permit 1.0.
    assert_eq!(result.score_ci, Some([0.0, 0.75]));
    assert_eq!(elo(0.0), "-infinity");
    assert_eq!(elo(1.0), "+infinity");
}
#[test]
fn boundary_and_constant_clusters_have_honest_uncertainty() {
    for score in [0.0, 0.5, 1.0] {
        let games: Vec<_> = (0..10)
            .flat_map(|p| [game(p, "w", Some(score)), game(p, "b", Some(score))])
            .collect();
        let result = summarize(&games, 10000, 42).unwrap();
        let [low, high] = result.score_ci.unwrap();
        assert!(high > low);
        assert!(result.method.contains("Hoeffding"));
        if score != 0.5 {
            assert!(result.local_elo_difference.unwrap().contains("unbounded"));
        }
    }
}
#[test]
fn incomplete_and_failed_pairs_excluded_not_drawn() {
    let games = [
        game(0, "w", Some(1.0)),
        game(0, "b", None),
        game(1, "w", Some(0.0)),
    ];
    let result = summarize(&games, 10000, 42).unwrap();
    assert_eq!(result.terminal_games, 2);
    assert_eq!(result.eligible_games, 0);
    assert_eq!(result.excluded_pairs, 2);
    assert!(result.score.is_none());
    assert!(summarize(
        &[game(0, "w", Some(1.0)), game(0, "w", Some(0.0))],
        10000,
        42
    )
    .is_err());
    let mut invalid = game(0, "w", Some(0.5));
    invalid.termination = "ply-cap".into();
    assert!(summarize(&[invalid], 10000, 42).is_err());
}
#[test]
fn model_protocol_rejects_teacher_fields() {
    assert!(
        serde_json::from_str::<Request>(r#"{"fen":"position","pv":["e2e4"],"score":32}"#).is_err()
    );
    assert!(serde_json::from_str::<Request>(r#"{"fen":"position"}"#).is_ok());
}
