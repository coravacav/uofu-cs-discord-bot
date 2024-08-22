use std::sync::Arc;

use color_eyre::eyre::Result;
use dashmap::DashMap;
use poise::serenity_prelude::UserId;

struct _LLMRunner {
    tx: crossbeam_channel::Sender<String>,
    last_prompt: DashMap<UserId, String>,
}

pub fn setup_llm(
) -> Result<crossbeam_channel::Sender<(Arc<String>, tokio::sync::oneshot::Sender<String>)>> {
    let config = bot_llm::LLMConfig::new()?;
    let (tx, rx) =
        crossbeam_channel::bounded::<(Arc<String>, tokio::sync::oneshot::Sender<String>)>(100);

    tokio::task::spawn_blocking(move || loop {
        while let Ok((prompt, return_channel)) = rx.recv() {
            tracing::info!("prompt: {}", prompt);
            let Ok(result) = bot_llm::run_it(config.clone(), &prompt) else {
                return_channel
                    .send("Error: LLM failed to run".to_owned())
                    .unwrap();
                continue;
            };
            tracing::info!("result: {:?}", result);
            return_channel.send(result).unwrap();
        }
    });

    Ok(tx)
}
