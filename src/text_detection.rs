use poise::serenity_prelude as serenity;
use poise::Event;
pub struct Data {} // User data, which is stored and accessible in all command invocations
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

pub async fn event_handler(
        ctx: &serenity::Context,
        event: &Event<'_>,
        _framework: poise::FrameworkContext<'_, Data, Error>,
        data: &Data,
) -> Result<(), Error> {
        match event {
                Event::Message { new_message } => {
                        if new_message.content.to_lowercase().contains("rust") && !new_message.author.bot {
                                new_message
                                        .reply(ctx, format!("RUST MENTIONED :crab: :crab: :crab:"))
                                        .await?;
                        }
                }
                _ => {}
        }
        Ok(())
}