use std::fmt;

use serde::{Deserialize, Serialize};

macro_rules! id_type {
    ($name:ident, $inner:ty) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub $inner);

        impl $name {
            #[must_use]
            pub const fn new(value: $inner) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn get(self) -> $inner {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

id_type!(TrackId, i64);
id_type!(AlbumId, i64);
id_type!(ArtistId, i64);
id_type!(PlaylistId, i64);
id_type!(QueueItemId, u64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_display_their_value() {
        assert_eq!(TrackId::new(42).to_string(), "42");
        assert_eq!(QueueItemId::new(7).get(), 7);
    }
}
