mod bank_admin;
mod bank_user;

pub use bank_admin::*;
pub use bank_user::*;
use bot_db::bank::Change;
use poise::serenity_prelude::{Mentionable, UserId};

fn build_history_message(history: impl DoubleEndedIterator<Item = Change>, user: UserId) -> String {
    let mut message_text = String::from("### History:\n");

    message_text.push_str(&user.mention().to_string());
    message_text.push('\n');

    for Change { amount, reason } in history.rev().take(20) {
        message_text.push_str(&format!("`{:>9}`: {}\n", amount, reason));
    }

    message_text
}
