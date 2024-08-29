use crate::{KingFisherDb, ReadWriteTree};
use color_eyre::eyre::Result;
use poise::serenity_prelude::{self as serenity};
use serde::{Deserialize, Serialize};
use sled::Tree;

#[derive(Debug, Deserialize, Serialize)]
pub struct Change {
    pub amount: i64,
    pub reason: String,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct BankAccount {
    pub balance: i64,
    pub changes: Vec<Change>,
}

pub struct BankDb(Tree);

impl BankDb {
    pub fn new(db: &KingFisherDb) -> Result<Self> {
        let db = db.open_tree("bank")?;

        fn increment(
            _key: &[u8],
            old_value: Option<&[u8]>,
            merged_bytes: &[u8],
        ) -> Option<Vec<u8>> {
            dbg!(_key, old_value, merged_bytes);

            KingFisherDb::create_update_with_deserialization::<BankAccount>(
                old_value,
                |mut account| {
                    let Ok(change) = bincode::deserialize::<Change>(merged_bytes) else {
                        tracing::error!("Failed to deserialize change, {:?}", merged_bytes);
                        return account;
                    };

                    dbg!(&change, &account);

                    account.balance += change.amount;
                    account.changes.push(change);
                    account
                },
                Default::default,
            )
        }

        db.set_merge_operator(increment);

        Ok(BankDb(db))
    }

    pub fn get(&self, user_id: serenity::UserId) -> Result<BankAccount> {
        let user_id: u64 = user_id.into();
        self.0.typed_get_or_default::<u64, BankAccount>(&user_id)
    }

    pub fn change(&self, user_id: serenity::UserId, amount: i64, reason: String) -> Result<()> {
        let user_id: u64 = user_id.into();
        let change = Change { amount, reason };
        self.0
            .typed_merge::<u64, Change>(&user_id, &change)
            .map(|_| {})
    }
}
