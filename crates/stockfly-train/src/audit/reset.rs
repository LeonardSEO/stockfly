use crate::checkpoint::Checkpoint;

/// Derives an in-memory baseline that retains checkpoint identity and
/// training metadata while removing every learned edge magnitude.
pub fn checkpoint_without_learning(checkpoint: &Checkpoint) -> Checkpoint {
    let mut reset = checkpoint.clone();
    reset.deltas.clear();
    reset
}
