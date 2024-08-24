use std::ops::Add;

use color_eyre::eyre::Result;
use sled::{Db, IVec, Tree};

pub fn get_lynch_leaderboard(db: &Db) -> Result<Tree> {
    Ok(db.open_tree("lynch_leaderboard")?)
}

pub fn get_db() -> Result<Db> {
    Ok(sled::open("kingfisher.db")?)
}

pub struct DbU64(u64);

impl DbU64 {
    pub fn new(value: u64) -> Self {
        Self(value)
    }
}

impl From<u64> for DbU64 {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<DbU64> for u64 {
    fn from(value: DbU64) -> Self {
        value.0
    }
}

impl From<[u8; 8]> for DbU64 {
    fn from(value: [u8; 8]) -> Self {
        Self(u64::from_ne_bytes(value))
    }
}

impl From<IVec> for DbU64 {
    fn from(value: IVec) -> Self {
        Self(u64::from_ne_bytes(value.as_ref().try_into().unwrap()))
    }
}

impl Add for DbU64 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Add<usize> for DbU64 {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0 + rhs as u64)
    }
}

impl From<DbU64> for IVec {
    fn from(value: DbU64) -> Self {
        (&value.0.to_ne_bytes()).into()
    }
}
