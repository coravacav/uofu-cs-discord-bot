use std::collections::HashMap;

use crate::{KingFisherDb, ReadWriteTree};
use color_eyre::eyre::Result;
use poise::serenity_prelude::{self as serenity, UserId};
use sled::Tree;

struct RussianRouletteGame {
    game_players: Vec<UserId>,
    loser: UserId,
}

struct RussianRouletteStat {
    games_played: u64,
    games_won: u64,
}

struct RussianRouletteGames {
    player_results: HashMap<UserId, RussianRouletteStat>,
    game_results: Vec<UserId>,
}

pub struct RussianRouletteStats(Tree);

impl RussianRouletteStats {
    pub fn new(db: &KingFisherDb) -> Result<Self> {
        let db = db.open_tree("russian_roulette_stats")?;

        fn increment(
            _key: &[u8],
            old_value: Option<&[u8]>,
            _merged_bytes: &[u8],
        ) -> Option<Vec<u8>> {
            KingFisherDb::create_update_with_deserialization::<usize>(
                old_value,
                |mut value| {
                    value += 1;
                    value
                },
                || 0,
            )
        }

        db.set_merge_operator(increment);

        Ok(RussianRouletteStats(db))
    }

    pub fn set(&self, user_id: serenity::UserId, count: u64) -> Result<()> {
        let user_id: u64 = user_id.into();
        self.0.typed_insert::<u64, u64>(&user_id, &count)
    }

    pub fn increment(&self, user_id: serenity::UserId) -> Result<Option<u64>> {
        let user_id: u64 = user_id.into();
        self.0.typed_merge(&user_id, &1u64)
    }

    pub fn get(&self, user_id: serenity::UserId) -> Result<Option<u64>> {
        let user_id: u64 = user_id.into();
        self.0.typed_get::<u64, u64>(&user_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (serenity::UserId, u64)> {
        self.0
            .iter()
            .filter_map(|i| i.ok())
            .map(|(user_id, count)| -> Result<(serenity::UserId, u64)> {
                let user_id: u64 = bincode::deserialize(&user_id)?;
                let count: u64 = bincode::deserialize(&count)?;
                Ok((serenity::UserId::from(user_id), count))
            })
            .filter_map(|i| i.ok())
    }
}
