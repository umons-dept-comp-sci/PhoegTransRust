use std::io;
use std::error;
use std::fmt;
use std::sync::mpsc;
use std::any::Any;
use rayon;

#[derive(Debug)]
pub enum TransProofError {
    Io(io::Error),
    Send(mpsc::SendError<String>),
    Thread(Box<Any + Send>),
    ThreadPool(rayon::ThreadPoolBuildError),
}

impl fmt::Display for TransProofError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            TransProofError::Io(ref e) => write!(f, "Io error : {}", e),
            TransProofError::Send(ref e) => write!(f, "Communication error : {}", e),
            TransProofError::Thread(ref e) => write!(f, "Thread error : {:?}", e),
            TransProofError::ThreadPool(ref e) => write!(f, "Thread pool error : {:?}", e),
        }
    }
}

impl error::Error for TransProofError {
    fn description(&self) -> &str {
        match *self {
            TransProofError::Io(ref e) => e.description(),
            TransProofError::Send(ref e) => e.description(),
            TransProofError::Thread(_) => "Data handling thread panicked.",
            TransProofError::ThreadPool(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            TransProofError::Io(ref e) => Some(e),
            TransProofError::Send(ref e) => Some(e),
            TransProofError::Thread(_) => Some(self),
            TransProofError::ThreadPool(ref e) => Some(e),
        }
    }
}

impl From<io::Error> for TransProofError {
    fn from(e: io::Error) -> TransProofError {
        TransProofError::Io(e)
    }
}

impl From<mpsc::SendError<String>> for TransProofError {
    fn from(e: mpsc::SendError<String>) -> TransProofError {
        TransProofError::Send(e)
    }
}

impl From<rayon::ThreadPoolBuildError> for TransProofError {
    fn from(e: rayon::ThreadPoolBuildError) -> TransProofError {
        TransProofError::ThreadPool(e)
    }
}

impl From<Box<Any + Send>> for TransProofError {
    fn from(e: Box<Any + Send>) -> TransProofError {
        TransProofError::Thread(e)
    }
}
