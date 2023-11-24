use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Starboard {
    pub reaction_count: u64,
    pub emote_name: String,
    pub channel_id: u64,
}
