mod add_bot_role;
mod create_class_category;
mod register;
mod remove_bot_role;

use add_bot_role::add_bot_role;
use create_class_category::create_class_category;
use register::register;
use remove_bot_role::remove_bot_role;

pub fn get_commands() -> Vec<poise::Command<crate::data::Data, anyhow::Error>> {
    vec![
        register(),
        create_class_category(),
        add_bot_role(),
        remove_bot_role(),
    ]
}
