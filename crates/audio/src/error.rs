use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum AudioError {
    #[error("audio output unavailable: {0}")]
    OutputUnavailable(String),
    #[error("audio output error: {0}")]
    Output(String),
    #[error("unsupported audio: {0}")]
    Unsupported(String),
    #[error("decode error: {0}")]
    Decode(String),
    #[error("audio file error: {0}")]
    Io(String),
    #[error("invalid seek position")]
    InvalidSeek,
    #[error("the audio engine is not running")]
    EngineStopped,
}
