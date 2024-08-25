use std::fmt::Debug;

use color_eyre::eyre::{Context, Result};
use poise::serenity_prelude as serenity;
use serde::{de::DeserializeOwned, Serialize};
use sled::{Db, Tree};

pub trait ReadWriteTree {
    fn typed_insert<K: DeserializeOwned + Serialize, V: DeserializeOwned + Serialize>(
        &self,
        key: &K,
        value: &V,
    ) -> Result<()>;

    fn typed_get<K: DeserializeOwned + Serialize, V: DeserializeOwned + Serialize>(
        &self,
        key: &K,
    ) -> Result<Option<V>>;

    fn typed_merge<K: DeserializeOwned + Serialize, V: DeserializeOwned + Serialize>(
        &self,
        key: &K,
        value: &V,
    ) -> Result<Option<V>>;
}

impl ReadWriteTree for Tree {
    fn typed_insert<K: DeserializeOwned + Serialize, V: DeserializeOwned + Serialize>(
        &self,
        key: &K,
        value: &V,
    ) -> Result<()> {
        let key = bincode::serialize::<K>(key)?;
        let value = bincode::serialize::<V>(value)?;
        self.insert(key, value)?;
        Ok(())
    }

    fn typed_get<K: DeserializeOwned + Serialize, V: DeserializeOwned + Serialize>(
        &self,
        key: &K,
    ) -> Result<Option<V>> {
        Ok(self
            .get(bincode::serialize::<K>(key)?)?
            .map(|value| bincode::deserialize::<V>(&value))
            .transpose()?)
    }

    fn typed_merge<K: DeserializeOwned + Serialize, V: DeserializeOwned + Serialize>(
        &self,
        key: &K,
        value: &V,
    ) -> Result<Option<V>> {
        Ok(self
            .merge(
                bincode::serialize::<K>(key)?,
                bincode::serialize::<V>(value)?,
            )?
            .map(|value| bincode::deserialize::<V>(&value))
            .transpose()?)
    }
}

pub struct LynchLeaderboard(Tree);

impl LynchLeaderboard {
    pub fn set(&self, user_id: serenity::UserId, count: u64) -> Result<()> {
        let user_id: u64 = user_id.into();
        self.0.typed_insert::<u64, u64>(&user_id, &count)
    }

    pub fn increment(&self, user_id: serenity::UserId) -> Result<Option<u64>> {
        let user_id: u64 = user_id.into();
        self.0.typed_merge(&user_id, &0u64)
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

fn create_update_with_deserialization<V: DeserializeOwned + Serialize + Debug>(
    old_value: Option<&[u8]>,
    update_function: impl FnMut(V) -> V,
    mut get_default_value: impl FnMut() -> V,
) -> Option<Vec<u8>> {
    old_value
        .map_or_else(
            || Ok(get_default_value()),
            |v| bincode::deserialize::<V>(v).wrap_err("Failed to deserialize"),
        )
        .inspect_err(|e| tracing::error!("{:?}", e))
        .ok()
        .map(update_function)
        .map(|new_value| bincode::serialize::<V>(&new_value).wrap_err("Failed to serialize"))
        .transpose()
        .inspect_err(|e| tracing::error!("{:?}", e))
        .ok()
        .flatten()
        .or_else(|| old_value.map(|v| v.to_vec()))
}

pub fn get_lynch_leaderboard(db: &Db) -> Result<LynchLeaderboard> {
    let db = db.open_tree("lynch_leaderboard")?;

    fn increment(_key: &[u8], old_value: Option<&[u8]>, _merged_bytes: &[u8]) -> Option<Vec<u8>> {
        create_update_with_deserialization::<usize>(
            old_value,
            |mut value| {
                value += 1;
                value
            },
            || 0,
        )
    }

    db.set_merge_operator(increment);

    Ok(LynchLeaderboard(db))
}

// struct MessageStore(Tree);

// impl MessageStore {
//     pub fn set(&self, user_id: serenity::UserId, message_id: serenity::MessageId) -> Result<()> {
//         let user_id: u64 = user_id.into();
//         self.0.typed_insert::<u64, u64>(&user_id, &message_id.0)
//     }
// }

// pub fn get_message_store(db: &Db) -> Result<MessageStore> {
//     let db = db.open_tree("message_store")?;

//     fn add_message(_key: &[u8], old_value: Option<&[u8]>, merged_bytes: &[u8]) -> Option<Vec<u8>> {
//         let mut value = old_value
//             .map_or(Ok(vec![]), bincode::deserialize::<Vec<&[u8]>>)
//             .ok()?;

//         value.push(merged_bytes);

//         bincode::serialize(&value)
//             .ok()
//             .or_else(|| old_value.map(|v| v.to_vec()))
//     }

//     Ok(MessageStore(db))
// }

pub fn get_db() -> Result<Db> {
    let db = sled::open("kingfisher.db")?;

    Ok(db)
}
