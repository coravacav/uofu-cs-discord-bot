pub mod yeet;

use std::fmt::Debug;

use bot_traits::ForwardRefToTracing;
use color_eyre::eyre::{Context, Result};
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

#[derive(Debug)]
pub struct KingFisherDb(Db);

impl KingFisherDb {
    pub fn new() -> Result<Self> {
        Ok(Self(sled::open("kingfisher.db")?))
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
            .trace_err_ok()
            .map(update_function)
            .map(|new_value| bincode::serialize::<V>(&new_value).wrap_err("Failed to serialize"))
            .transpose()
            .trace_err_ok()
            .flatten()
            .or_else(|| old_value.map(|v| v.to_vec()))
    }

    fn open_tree(&self, name: impl AsRef<[u8]>) -> Result<Tree> {
        self.0.open_tree(name).wrap_err("Failed to open tree")
    }
}

pub fn perform_migration() -> Result<()> {
    Ok(())
}
