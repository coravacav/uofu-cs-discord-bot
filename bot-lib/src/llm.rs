use color_eyre::eyre::Result;

type LLMTxValue = (String, tokio::sync::oneshot::Sender<String>);
type LLMTx = crossbeam_channel::Sender<LLMTxValue>;

#[derive(Debug)]
pub struct LLMS {
    pub big: LLMTx,
    pub small: LLMTx,
}

const BIG_MODEL_PATH: &str = "llms/big/Meta-Llama-3.1-8B-Instruct-Q8_0.gguf";
const BIG_TOKENIZER_PATH: &str = "llms/big/tokenizer.json";
const SMALL_MODEL_PATH: &str = "llms/small/llama-3.2-3b-instruct-q8_0.gguf";
const SMALL_TOKENIZER_PATH: &str = "llms/small/tokenizer.json";

fn setup_one_llm(_model_path: &'static str, _tokenizer_path: &'static str) -> Result<LLMTx> {
    let (tx, rx) = crossbeam_channel::bounded::<LLMTxValue>(100);

    tokio::task::spawn_blocking(move || -> Result<()> {
        // let mut model = bot_llm::load_model(model_path)?;
        // let tokenizer = bot_llm::load_tokenizer(tokenizer_path)?;

        while let Ok((_prompt, return_channel)) = rx.recv() {
            // tracing::info!("prompt: {}", prompt);
            // let now = std::time::Instant::now();
            // let Ok(result) = bot_llm::run_the_model(&mut model, &tokenizer, &prompt) else {
            return_channel
                .send("LLM is currently disabled, haven't fixed it yet 👍".to_owned())
                .ok();
            //     continue;
            // };
            // tracing::info!("result: {:?}", result);
            // tracing::info!("LLM took {}ms", now.elapsed().as_millis());
            // return_channel.send(result).ok();
        }

        Ok(())
    });

    Ok(tx)
}

pub fn setup_llms() -> Result<LLMS> {
    Ok(LLMS {
        big: setup_one_llm(BIG_MODEL_PATH, BIG_TOKENIZER_PATH)?,
        small: setup_one_llm(SMALL_MODEL_PATH, SMALL_TOKENIZER_PATH)?,
    })
}
