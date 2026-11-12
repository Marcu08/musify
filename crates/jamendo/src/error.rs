/// Errors from the Jamendo source. Messages never contain the client id or the
/// request URL: the client id is a query parameter and must not leak into logs.
#[derive(Debug, thiserror::Error)]
pub enum JamendoError {
    #[error("Jamendo network error: {0}")]
    Network(String),
    #[error("Jamendo API error: {0}")]
    Api(String),
    #[error("Jamendo storage error: {0}")]
    Storage(String),
    #[error("Jamendo track not found: {0}")]
    NotFound(String),
}

impl From<rusqlite::Error> for JamendoError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Storage(error.to_string())
    }
}
