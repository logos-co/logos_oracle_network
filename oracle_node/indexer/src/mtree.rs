use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Copy)]
pub struct TreeSnapshot {
    pub root: [u8; 32],
    pub total_leaves: usize,
}

#[derive(Default)]
pub struct RootsCache {
    pub history: Arc<RwLock<VecDeque<TreeSnapshot>>>,
    // Max number of root to keep
    pub max_capacity: usize,
}