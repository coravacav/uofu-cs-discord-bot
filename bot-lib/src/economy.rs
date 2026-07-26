use crate::data::DB;
use color_eyre::eyre::{Result, eyre};
use poise::serenity_prelude::UserId;
use serde::Deserialize;
use surrealdb::types::{RecordId, SurrealValue};

#[derive(Clone, Debug, Deserialize, SurrealValue)]
pub struct Change {
    pub amount: i64,
    pub reason: String,
}

#[derive(Clone, Debug, Default, Deserialize, SurrealValue)]
pub struct BankAccount {
    pub balance: i64,
    pub changes: Vec<Change>,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct BankRanking {
    user_id: i64,
    balance: i64,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct YeetScore {
    user_id: i64,
    count: u64,
}

fn record_id(table: &str, user_id: UserId) -> Result<RecordId> {
    let user_id = i64::try_from(u64::from(user_id))
        .map_err(|_| eyre!("Discord user ID does not fit in a SurrealDB numeric record ID"))?;
    Ok(RecordId::new(table, user_id))
}

pub struct Bank;

impl Bank {
    pub async fn get(user_id: UserId) -> Result<BankAccount> {
        let account = DB
            .select::<Option<BankAccount>>(record_id("bank_account", user_id)?)
            .await?;
        Ok(account.unwrap_or_default())
    }

    pub async fn change(user_id: UserId, amount: i64, reason: String) -> Result<BankAccount> {
        let account = record_id("bank_account", user_id)?;
        let mut response = DB
            .query(
                "UPSERT ONLY $account \
                 SET balance += $amount, changes += { amount: $amount, reason: $reason } \
                 RETURN AFTER",
            )
            .bind(("account", account))
            .bind(("amount", amount))
            .bind(("reason", reason))
            .await?
            .check()?;

        response
            .take::<Option<BankAccount>>(0)?
            .ok_or_else(|| eyre!("bank account UPSERT returned no record"))
    }

    pub async fn get_history(user_id: UserId) -> Result<Option<Vec<Change>>> {
        Ok(DB
            .select::<Option<BankAccount>>(record_id("bank_account", user_id)?)
            .await?
            .map(|account| account.changes))
    }

    pub async fn global_rankings() -> Result<Vec<(UserId, BankAccount)>> {
        let rankings: Vec<BankRanking> = DB
            .query(
                "SELECT record::id(id) AS user_id, balance \
                 FROM bank_account ORDER BY balance DESC",
            )
            .await?
            .check()?
            .take(0)?;

        rankings
            .into_iter()
            .map(|ranking| {
                let user_id = u64::try_from(ranking.user_id)
                    .map(UserId::new)
                    .map_err(|_| eyre!("invalid bank account user ID {}", ranking.user_id))?;
                Ok((
                    user_id,
                    BankAccount {
                        balance: ranking.balance,
                        changes: Vec::new(),
                    },
                ))
            })
            .collect()
    }
}

pub struct YeetLeaderboard;

impl YeetLeaderboard {
    pub async fn increment(user_id: UserId) -> Result<u64> {
        let score = record_id("yeet_score", user_id)?;
        let mut response = DB
            .query("UPSERT ONLY $score SET count += 1 RETURN AFTER")
            .bind(("score", score))
            .await?
            .check()?;

        response
            .take::<Option<YeetScoreRecord>>(0)?
            .map(|score| score.count)
            .ok_or_else(|| eyre!("yeet score UPSERT returned no record"))
    }

    pub async fn rankings() -> Result<Vec<(UserId, u64)>> {
        let rankings: Vec<YeetScore> = DB
            .query(
                "SELECT record::id(id) AS user_id, count \
                 FROM yeet_score ORDER BY count DESC",
            )
            .await?
            .check()?
            .take(0)?;

        rankings
            .into_iter()
            .map(|score| {
                let user_id = u64::try_from(score.user_id)
                    .map(UserId::new)
                    .map_err(|_| eyre!("invalid yeet score user ID {}", score.user_id))?;
                Ok((user_id, score.count))
            })
            .collect()
    }
}

#[derive(Debug, Deserialize, SurrealValue)]
struct YeetScoreRecord {
    count: u64,
}

#[cfg(test)]
mod tests {
    use super::{Bank, YeetLeaderboard};
    use crate::data::setup_db;
    use poise::serenity_prelude::UserId;

    #[tokio::test]
    async fn economy_is_persisted_and_ranked() {
        setup_db().await;

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
