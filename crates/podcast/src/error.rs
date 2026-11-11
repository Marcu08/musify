use thiserror::Error;

#[derive(Debug, Error)]
pub enum PodcastError {
    #[error("podcast network error: {0}")]
    Network(String),
    #[error("podcast storage error: {0}")]
    Storage(String),
    #[error("invalid podcast feed: {0}")]
    Feed(String),
    #[error("podcast item not found: {0}")]
    NotFound(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_have_ui_ready_messages() {
        assert!(
            PodcastError::Network("offline".into())
                .to_string()
                .contains("offline")
        );
    }
}
