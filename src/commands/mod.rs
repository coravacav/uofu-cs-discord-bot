mod add_bot_role;
mod create_class_category;
mod register;
mod remove_bot_role;
mod timeout;

use color_eyre::eyre::Error;

pub fn get_commands() -> Vec<poise::Command<crate::data::AppState, Error>> {
    vec![
        add_bot_role::add_bot_role(),
        create_class_category::create_class_category(),
        register::register(),
        remove_bot_role::remove_bot_role(),
        timeout::timeout(),
    ]
}
