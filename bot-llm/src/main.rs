use bot_llm::{load_model, run_the_model};
use color_eyre::eyre::Result;

fn main() -> Result<()> {
    let mut model = load_model()?;
    run_the_model(&mut model, "What's your favorite color?")?;
    run_the_model(&mut model, "What's your favorite color?")?;

    Ok(())
}
