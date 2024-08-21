use color_eyre::eyre::Result;
use dashmap::DashMap;
use poise::serenity_prelude::UserId;

struct _LLMRunner {
    tx: crossbeam_channel::Sender<String>,
    last_prompt: DashMap<UserId, String>,
}

pub fn setup_llm(
) -> Result<crossbeam_channel::Sender<(String, tokio::sync::oneshot::Sender<String>)>> {
    let config = bot_llm::LLMConfig::new()?;
    let (tx, rx) =
        crossbeam_channel::bounded::<(String, tokio::sync::oneshot::Sender<String>)>(100);

    tokio::task::spawn_blocking(move || loop {
        while let Ok((prompt, _return_channel)) = rx.recv() {
            tracing::info!("prompt: {}", prompt);
            let result = bot_llm::run_it(config.clone(), &prompt);
            tracing::info!("result: {:?}", result);
        }
    });

    Ok(tx)
}
