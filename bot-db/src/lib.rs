pub mod bank;
pub mod yeet;

use bot_traits::ForwardRefToTracing;
use color_eyre::eyre::{Context, Result};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sled::{Db, Tree};
use std::fmt::Debug;

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

    fn typed_get_or_default<
        K: DeserializeOwned + Serialize,
        V: DeserializeOwned + Serialize + Default,
    >(
        &self,
        key: &K,
    ) -> Result<V>;

    fn typed_merge<
        K: DeserializeOwned + Serialize,
        InsertedValue: DeserializeOwned + Serialize,
        ReturnedValue: DeserializeOwned + Serialize,
    >(
        &self,
        key: &K,
        value: &InsertedValue,
    ) -> Result<Option<ReturnedValue>>;

    fn typed_iter<K: DeserializeOwned + Serialize, V: DeserializeOwned + Serialize>(
        &self,
    ) -> Result<impl Iterator<Item = (K, V)>>;
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

    fn typed_get_or_default<
        K: DeserializeOwned + Serialize,
        V: DeserializeOwned + Serialize + Default,
    >(
        &self,
        key: &K,
    ) -> Result<V> {
        Ok(self
            .get(bincode::serialize::<K>(key).context("Failed to serialize key")?)
            .trace_err_ok()
            .flatten()
            .map(|value| bincode::deserialize::<V>(&value))
            .transpose()
            .ok()
            .flatten()
            .unwrap_or_default())
    }

    fn typed_merge<
        K: DeserializeOwned + Serialize,
        InsertedValue: DeserializeOwned + Serialize,
        ReturnedValue: DeserializeOwned + Serialize,
    >(
        &self,
        key: &K,
        value: &InsertedValue,
    ) -> Result<Option<ReturnedValue>> {
        Ok(self
            .merge(
                bincode::serialize::<K>(key)?,
                bincode::serialize::<InsertedValue>(value)?,
            )?
            .map(|value| bincode::deserialize::<ReturnedValue>(&value))
            .transpose()?)
    }

    fn typed_iter<K: DeserializeOwned + Serialize, V: DeserializeOwned + Serialize>(
        &self,
    ) -> Result<impl Iterator<Item = (K, V)>> {
        Ok(self
            .iter()
            .filter_map(|i| i.trace_err_ok())
            .map(|(key, value)| Ok((bincode::deserialize(&key)?, bincode::deserialize(&value)?)))
            .filter_map(|i: Result<(K, V)>| i.trace_err_ok()))
    }
}

pub struct KingFisherDb(Db);

impl KingFisherDb {
    pub fn new() -> Result<Self> {
        Ok(Self(sled::open("kingfisher.db")?))
    }

    fn create_update_with_deserialization<V: DeserializeOwned + Serialize>(
        old_value: Option<&[u8]>,
        update_function: impl FnMut(V) -> V,
        mut get_default_value: impl FnMut() -> V,
    ) -> Option<Vec<u8>> {
        old_value
            .map_or_else(
                || Ok(get_default_value()),
                |v| bincode::deserialize::<V>(v).context("Failed to deserialize"),
            )
            .trace_err_ok()
            .map(update_function)
            .map(|new_value| bincode::serialize::<V>(&new_value).context("Failed to serialize"))
            .transpose()
            .trace_err_ok()
            .flatten()
            .or_else(|| old_value.map(|v| v.to_vec()))
    }

    fn open_tree(&self, name: impl AsRef<[u8]>) -> Result<Tree> {
        self.0.open_tree(name).wrap_err("Failed to open tree")
    }

    pub fn debug_remove_value<K: DeserializeOwned + Serialize + Debug>(
        &self,
        tree: &str,
        key: &K,
    ) -> Result<()> {
        let key = bincode::serialize::<K>(key)?;
        let db = self.open_tree(tree)?;
        db.remove(&key)?;
        Ok(())
    }

    pub fn debug_inspect_value<K: DeserializeOwned + Serialize + Debug>(
        &self,
        tree: &str,
        key: &K,
    ) -> Result<Option<String>> {
        let key = bincode::serialize::<K>(key)?;
        let db = self.open_tree(tree)?;
        Ok(db.get(&key)?.map(|v| format!("{:?}", v)))
    }
}

#[allow(dead_code)]
fn perform_migration<
    OldValue: DeserializeOwned + Serialize + Debug,
    NewValue: DeserializeOwned + Serialize + Debug,
    OldKey: DeserializeOwned + Serialize + Debug,
    NewKey: DeserializeOwned + Serialize + Debug,
>(
    tree: &Tree,
    version_check_function: impl Fn(&OldValue) -> bool,
    mut update_function: impl FnMut(OldKey, OldValue) -> (NewKey, NewValue),
) -> Result<()> {
    // Store last
    for data in tree.iter() {
        let Ok((old_key_bytes, old_value_bytes)) = data else {
            continue;
        };

        let Ok(old_key) = bincode::deserialize::<OldKey>(&old_key_bytes) else {
            eprintln!("Failed to deserialize old key, {:?}", old_key_bytes);
            continue;
        };

        let Ok(old_value) = bincode::deserialize::<OldValue>(&old_value_bytes) else {
            eprintln!("Failed to deserialize old value, key={:?}", old_key);
            continue;
        };

        if !version_check_function(&old_value) {
            continue;
        }

        let (new_key, new_value) = update_function(old_key, old_value);

        let new_key_bytes = bincode::serialize::<NewKey>(&new_key)?;
        let new_value_bytes = bincode::serialize::<NewValue>(&new_value)?;

        if *new_key_bytes != *old_key_bytes {
            tree.remove(&old_key_bytes)?;
        }

        tree.insert(new_key_bytes, new_value_bytes)?;
    }

    Ok(())
}

#[derive(Deserialize, Serialize)]
struct DataWithVersion<T> {
    version: u32,
    data: T,
}

impl<T> DataWithVersion<T> {
    #[allow(dead_code)]
    fn new(version: u32, data: T) -> Self {
        Self { version, data }
    }
}
