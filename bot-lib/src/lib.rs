use color_eyre::eyre::Result;
use data::PoiseContext;

pub mod commands;
pub mod config;
pub mod courses;
pub mod data;
pub mod event_handler;
mod handle_starboards;
mod lang;
pub mod llm;
mod starboard;
mod text_detection;
mod utils;

pub trait SayThenDelete {
    async fn say_then_delete(self, message: impl Into<String>) -> Result<()>;
}

impl<'a> SayThenDelete for PoiseContext<'a> {
    async fn say_then_delete(self, message: impl Into<String>) -> Result<()> {
        let message = self.say(message).await?;

        tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        message.delete(self).await.ok();

        Ok(())
    }
}
