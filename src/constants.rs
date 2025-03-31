use std::sync::OnceLock;

pub static NUM_BEST: OnceLock<usize> = OnceLock::new();