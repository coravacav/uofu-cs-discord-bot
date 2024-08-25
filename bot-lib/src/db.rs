use color_eyre::eyre::Result;
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

pub fn get_lynch_leaderboard(db: &Db) -> Result<LynchLeaderboard> {
    let db = db.open_tree("lynch_leaderboard")?;

    fn increment(_key: &[u8], old_value: Option<&[u8]>, _merged_bytes: &[u8]) -> Option<Vec<u8>> {
        let old_value = old_value.map_or(Ok(0), bincode::deserialize::<u64>).ok()?;
        let new_value = old_value + 1;
        let ret = bincode::serialize(&new_value).ok()?;

        Some(ret)
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

//     Ok(MessageStore(db))
// }

pub fn get_db() -> Result<Db> {
    let db = sled::open("kingfisher.db")?;

    Ok(db)
}
