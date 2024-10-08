use color_eyre::eyre::Result;

struct _LLMRunner {
    tx: crossbeam_channel::Sender<String>,
}

type LLMTxValue = (String, tokio::sync::oneshot::Sender<String>);
type LLMTx = crossbeam_channel::Sender<LLMTxValue>;

pub fn setup_llm() -> Result<LLMTx> {
    let (tx, rx) = crossbeam_channel::bounded::<LLMTxValue>(100);

    tokio::task::spawn_blocking(move || -> Result<()> {
        let mut model = match bot_llm::load_model() {
            Ok(model) => model,
            Err(e) => {
                tracing::warn!("Failed to create LLM config, the commands will not work: {e}");
                while let Ok((_, return_channel)) = rx.recv() {
                    return_channel
                        .send("Error: LLM failed to run".to_owned())
                        .ok();
                }

                return Ok(());
            }
        };

        while let Ok((prompt, return_channel)) = rx.recv() {
            tracing::info!("prompt: {}", prompt);
            let Ok(result) = bot_llm::run_the_model(&mut model, &prompt) else {
                return_channel
                    .send("Error: LLM failed to run".to_owned())
                    .ok();
                continue;
            };
            tracing::info!("result: {:?}", result);
            return_channel.send(result).ok();
        }

        Ok(())
    });

    Ok(tx)
}
