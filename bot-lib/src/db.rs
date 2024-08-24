use color_eyre::eyre::Result;
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

pub fn get_lynch_leaderboard(db: &Db) -> Result<Tree> {
    Ok(db.open_tree("lynch_leaderboard")?)
}

pub fn get_db() -> Result<Db> {
    let db = sled::open("kingfisher.db")?;

    // for (key, value) in db
    //     .open_tree("lynch_leaderboard")?
    //     .iter()
    //     .filter_map(|t| t.ok())
    // {
    //     let key = bincode::deserialize::<String>(&key)?;
    //     let value = bincode::deserialize::<u64>(&value)?;
    //     println!("{}: {}", key, value);
    // }

    Ok(db)
}
