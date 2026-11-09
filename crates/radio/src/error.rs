use thiserror::Error;

#[derive(Debug, Error)]
pub enum RadioError {
    #[error("radio network error: {0}")]
    Network(String),
    #[error("radio cache error: {0}")]
    Database(String),
    #[error("invalid radio-browser response: {0}")]
    Json(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_have_ui_ready_messages() {
        assert!(
            RadioError::Network("offline".into())
                .to_string()
                .contains("offline")
        );
    }
}
