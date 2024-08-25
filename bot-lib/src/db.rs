use color_eyre::eyre::Result;
use poise::serenity_prelude as serenity;
use serde::{de::DeserializeOwned, Serialize};
use sled::{Db, Tree};

pub trait ReadWriteTree {
    fn typed_insert<K: Serialize + ?Sized, V: Serialize + ?Sized>(
        &self,
        key: &K,
        value: &V,
    ) -> Result<()>;

    fn typed_get<K: Serialize, V: DeserializeOwned>(&self, key: &K) -> Result<Option<V>>;
}

impl ReadWriteTree for Tree {
    fn typed_insert<K: Serialize + ?Sized, V: Serialize + ?Sized>(
        &self,
        key: &K,
        value: &V,
    ) -> Result<()> {
        let key = bincode::serialize::<K>(key)?;
        let value = bincode::serialize::<V>(value)?;
        self.insert(key, value)?;
        Ok(())
    }

    fn typed_get<K: Serialize, V: DeserializeOwned>(&self, key: &K) -> Result<Option<V>> {
        let Some(value) = self.get(bincode::serialize::<K>(key)?)? else {
            return Ok(None);
        };
        let value = bincode::deserialize::<V>(&value)?;
        Ok(Some(value))
    }
}

pub struct LynchLeaderboard(Tree);

impl LynchLeaderboard {
    pub fn set(&self, user_id: serenity::UserId, count: u64) -> Result<()> {
        let user_id: u64 = user_id.into();
        self.0.typed_insert::<u64, u64>(&user_id, &count)
    }

    pub fn get(&self, user_id: serenity::UserId) -> Result<Option<u64>> {
        let user_id: u64 = user_id.into();
        self.0.typed_get::<u64, u64>(&user_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (serenity::UserId, u64)> {
        self.0
            .iter()
            .filter_map(|i| i.ok())
            .map(|(user_id, count)| {
                let user_id: u64 = bincode::deserialize(&user_id).unwrap();
                let count: u64 = bincode::deserialize(&count).unwrap();
                (serenity::UserId::from(user_id), count)
            })
    }
}

pub fn get_lynch_leaderboard(db: &Db) -> Result<LynchLeaderboard> {
    Ok(LynchLeaderboard(db.open_tree("lynch_leaderboard")?))
}

pub fn get_db() -> Result<Db> {
    let db = sled::open("kingfisher.db")?;

    Ok(db)
}
