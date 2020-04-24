use std::io;
use std::sync::mpsc;
use std::any::Any;
use rayon;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TransProofError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Send(#[from] mpsc::SendError<String>),
    #[error("Data handling thread panicked.")]
    Thread(Box<dyn Any + Send>),
    #[error(transparent)]
    ThreadPool(#[from] rayon::ThreadPoolBuildError),
}
