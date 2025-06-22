use lazy_static::lazy_static;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Duration;

pub static NUM_BEST: OnceLock<usize> = OnceLock::new();

lazy_static! {
    pub static ref TOTAL_TIME: Arc<Mutex<Duration>> = Arc::new(Mutex::new(Duration::default()));
    pub static ref SOUFFLE_TIME: Arc<Mutex<Duration>> = Arc::new(Mutex::new(Duration::default()));
    pub static ref NEO4J_TIME: Arc<Mutex<Duration>> = Arc::new(Mutex::new(Duration::default()));
    pub static ref SIM_TIME: Arc<Mutex<Duration>> = Arc::new(Mutex::new(Duration::default()));
    pub static ref GEN_TIME: Arc<Mutex<Duration>> = Arc::new(Mutex::new(Duration::default()));
    pub static ref AUTOMATON_TIME: Arc<Mutex<Duration>> = Arc::new(Mutex::new(Duration::default()));
}
