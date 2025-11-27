use bot_db::{KingFisherDb, bank::BankDb, yeet::YeetLeaderboard};
use color_eyre::eyre::Result;

fn main() -> Result<()> {
    let db = KingFisherDb::new()?;

    let bank = BankDb::new(&db)?;

    for (user_id, account) in bank.iter_all()? {
        println!("{}: {}", user_id, account.balance);
    }

    println!("Yeet Leaderboard");

    let yeet = YeetLeaderboard::connect(&db)?;

    for (user_id, count) in yeet.iter() {
        println!("{user_id}: {count}");
    }

    Ok(())
}
