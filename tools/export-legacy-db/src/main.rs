use serde::Deserialize;
use sled::Tree;
use std::{error::Error, path::PathBuf};

#[derive(Deserialize)]
struct Change {
    amount: i64,
    reason: String,
}

#[derive(Deserialize)]
struct BankAccount {
    balance: i64,
    changes: Vec<Change>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("kingfisher.db"));

    let database = sled::open(&path).map_err(|error| {
        format!(
            "failed to open {}: {error}; stop the bot before exporting",
            path.display()
        )
    })?;

    eprintln!("Exporting legacy economy data from {}", path.display());
    println!("BEGIN TRANSACTION;");

    export_bank(database.open_tree("bank")?)?;
    export_yeet_scores(database.open_tree("yeet_leaderboard")?)?;

    println!("COMMIT TRANSACTION;");
    eprintln!("Export complete");
    Ok(())
}

fn export_bank(tree: Tree) -> Result<(), Box<dyn Error>> {
    for entry in &tree {
        let (key, value) = entry?;
        let user_id: u64 = bincode::deserialize(&key)?;
        ensure_record_id_fits(user_id)?;
        let account: BankAccount = bincode::deserialize(&value)?;

        let changes = account
            .changes
            .into_iter()
            .map(|change| {
                Ok(format!(
                    "{{ amount: {}, reason: {} }}",
                    change.amount,
                    serde_json::to_string(&change.reason)?
                ))
            })
            .collect::<Result<Vec<_>, serde_json::Error>>()?
            .join(", ");

        println!(
            "UPSERT bank_account:{user_id} CONTENT {{ balance: {}, changes: [{changes}] }};",
            account.balance
        );
    }
    Ok(())
}

fn export_yeet_scores(tree: Tree) -> Result<(), Box<dyn Error>> {
    for entry in &tree {
        let (key, value) = entry?;
        let user_id: u64 = bincode::deserialize(&key)?;
        ensure_record_id_fits(user_id)?;
        let count: u64 = bincode::deserialize(&value)?;

        println!("UPSERT yeet_score:{user_id} CONTENT {{ count: {count} }};");
    }
    Ok(())
}

fn ensure_record_id_fits(user_id: u64) -> Result<(), Box<dyn Error>> {
    i64::try_from(user_id).map(|_| ()).map_err(|_| {
        format!("Discord user ID {user_id} is too large for a numeric record ID").into()
    })
}
