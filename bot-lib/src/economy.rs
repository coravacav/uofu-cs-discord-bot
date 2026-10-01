use crate::data::with_db;
use color_eyre::eyre::{OptionExt, Result};
use poise::serenity_prelude::UserId;
use rusqlite::{Connection, OptionalExtension, params};

#[derive(Clone, Debug)]
pub struct Change {
    pub amount: i64,
    pub reason: String,
}

#[derive(Clone, Debug, Default)]
pub struct BankAccount {
    pub balance: i64,
    pub changes: Vec<Change>,
}

fn load_account(conn: &Connection, user_id: u64) -> Result<Option<BankAccount>> {
    let Some(balance) = conn
        .query_row(
            "SELECT balance FROM bank_account WHERE user_id = ?1",
            [user_id],
            |row| row.get(0),
        )
        .optional()?
    else {
        return Ok(None);
    };

    let changes = conn
        .prepare_cached("SELECT amount, reason FROM bank_change WHERE user_id = ?1 ORDER BY id")?
        .query_map([user_id], |row| {
            Ok(Change {
                amount: row.get(0)?,
                reason: row.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    Ok(Some(BankAccount { balance, changes }))
}

pub struct Bank;

impl Bank {
    pub async fn get(user_id: UserId) -> Result<BankAccount> {
        let user_id = u64::from(user_id);
        with_db(move |conn| Ok(load_account(conn, user_id)?.unwrap_or_default())).await
    }

    pub async fn change(user_id: UserId, amount: i64, reason: String) -> Result<BankAccount> {
        let user_id = u64::from(user_id);
        with_db(move |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO bank_account (user_id, balance) VALUES (?1, ?2) \
                 ON CONFLICT (user_id) DO UPDATE SET balance = balance + excluded.balance",
                params![user_id, amount],
            )?;
            tx.execute(
                "INSERT INTO bank_change (user_id, amount, reason) VALUES (?1, ?2, ?3)",
                params![user_id, amount, reason],
            )?;
            let account =
                load_account(&tx, user_id)?.ok_or_eyre("bank account upsert left no record")?;
            tx.commit()?;
            Ok(account)
        })
        .await
    }

    pub async fn get_history(user_id: UserId) -> Result<Option<Vec<Change>>> {
        let user_id = u64::from(user_id);
        with_db(move |conn| Ok(load_account(conn, user_id)?.map(|account| account.changes))).await
    }

    pub async fn global_rankings() -> Result<Vec<(UserId, BankAccount)>> {
        with_db(|conn| {
            let rankings = conn
                .prepare("SELECT user_id, balance FROM bank_account ORDER BY balance DESC")?
                .query_map([], |row| {
                    Ok((
                        UserId::new(row.get(0)?),
                        BankAccount {
                            balance: row.get(1)?,
                            changes: Vec::new(),
                        },
                    ))
                })?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rankings)
        })
        .await
    }
}

pub struct YeetLeaderboard;

impl YeetLeaderboard {
    pub async fn increment(user_id: UserId) -> Result<u64> {
        let user_id = u64::from(user_id);
        with_db(move |conn| {
            Ok(conn.query_row(
                "INSERT INTO yeet_score (user_id, count) VALUES (?1, 1) \
                 ON CONFLICT (user_id) DO UPDATE SET count = count + 1 \
                 RETURNING count",
                [user_id],
                |row| row.get(0),
            )?)
        })
        .await
    }

    pub async fn rankings() -> Result<Vec<(UserId, u64)>> {
        with_db(|conn| {
            let rankings = conn
                .prepare("SELECT user_id, count FROM yeet_score ORDER BY count DESC")?
                .query_map([], |row| Ok((UserId::new(row.get(0)?), row.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rankings)
        })
        .await
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{Bank, YeetLeaderboard};
    use poise::serenity_prelude::UserId;

    pub(crate) async fn assert_economy_is_persisted_and_ranked() {
        let first = UserId::new(91_001);
        let second = UserId::new(91_002);

        assert_eq!(Bank::get(first).await.unwrap().balance, 0);
        assert_eq!(
            Bank::change(first, 5, "income".to_owned())
                .await
                .unwrap()
                .balance,
            5
        );
        assert_eq!(
            Bank::change(first, -2, "gamble".to_owned())
                .await
                .unwrap()
                .balance,
            3
        );
        Bank::change(second, 8, "income".to_owned()).await.unwrap();

        let history = Bank::get_history(first).await.unwrap().unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[1].amount, -2);

        let rankings = Bank::global_rankings().await.unwrap();
        assert_eq!(rankings[0].0, second);
        assert_eq!(rankings[1].0, first);

        assert_eq!(YeetLeaderboard::increment(first).await.unwrap(), 1);
        assert_eq!(YeetLeaderboard::increment(first).await.unwrap(), 2);
        assert_eq!(YeetLeaderboard::increment(second).await.unwrap(), 1);
        assert_eq!(YeetLeaderboard::rankings().await.unwrap()[0], (first, 2));
    }
}
