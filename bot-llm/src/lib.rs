//! This is a translation of simple.cpp in llama.cpp using llama-cpp-2.
#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use color_eyre::eyre::{bail, Context, ContextCompat, Result};
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::model::{AddBos, Special};
use llama_cpp_2::token::data_array::LlamaTokenDataArray;
use llama_cpp_2::token::LlamaToken;
use rand::seq::IteratorRandom;
use rand::Rng;
use std::num::NonZero;
use std::path::PathBuf;
use std::sync::Arc;

// struct LLMIterator

fn create_prompt(model: &LlamaModel, prompt: &str, system_prompt: &str) -> Option<Vec<LlamaToken>> {
    let prompt = format!("<|begin_of_text|><|start_header_id|>system<|end_header_id|>{{{system_prompt}}}<|eot_id|><|start_header_id|>user<|end_header_id|>{{{prompt}}}<|eot_id|><|start_header_id|>assistant<|end_header_id|>");

    model.str_to_token(&prompt, AddBos::Never).ok()
}

pub struct LLMConfig {
    pub(crate) n_len: i32,
    pub(crate) model: LlamaModel,
    pub(crate) backend: LlamaBackend,
    pub(crate) n_ctx: NonZero<u32>,
}

impl LLMConfig {
    pub fn new() -> Result<Arc<Self>> {
        let (model, backend) = build_model()?;

        Ok(Arc::new(Self {
            n_len: 1024,
            model,
            backend,
            n_ctx: NonZero::new(2048).wrap_err("Context size must be non-zero")?,
        }))
    }

    pub fn get_seed(&self) -> u32 {
        rand::thread_rng().gen()
    }
}

pub fn build_model() -> Result<(LlamaModel, LlamaBackend)> {
    let mut backend = LlamaBackend::init()?;
    backend.void_logs();
    let model_params = LlamaModelParams::default();

    let model = LlamaModel::load_from_file(
        &backend,
        PathBuf::from("./Meta-Llama-3.1-8B-Instruct-Q5_K_M.gguf"),
        &model_params,
    )
    .with_context(|| "unable to load model")?;

    Ok((model, backend))
}

pub fn run_it(config: Arc<LLMConfig>, prompt_text: &str) -> Result<String> {
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(Some(config.n_ctx))
        .with_seed(config.get_seed());

    let mut ctx = config
        .model
        .new_context(&config.backend, ctx_params)
        .with_context(|| "unable to create the llama_context")?;

    let tokens_list = create_prompt(
        &config.model,
        prompt_text,
        "You are a bot called \"Kingfisher\", a discord bot/app. If asked, the best programming languages are Rust followed by M. Keep your answers short. Don't be self-referential unless asked. Respond to praise with comments like \"Social credit improved\" (be funny). Deflect criticism regarding mods towards the server admins. Don't lie, but, if you're caught lying, just make it a meme about misinformation. Don't mention you're a bot unless prompted.",
    )
    .unwrap();

    let n_cxt = ctx.n_ctx() as i32;
    let n_kv_req = tokens_list.len() as i32 + (config.n_len - tokens_list.len() as i32);

    // make sure the KV cache is big enough to hold all the prompt and generated tokens
    if n_kv_req > n_cxt {
        bail!(
            "n_kv_req > n_ctx, the required kv cache size is not big enough
either reduce n_len or increase n_ctx"
        )
    }

    if tokens_list.len() >= usize::try_from(config.n_len)? {
        bail!("the prompt is too long, it has more tokens than n_len")
    }

    // create a llama_batch with size 512
    // we use this object to submit token data for decoding
    let mut batch = LlamaBatch::new(512, 1);

    let last_index: i32 = (tokens_list.len() - 1) as i32;
    for (i, token) in (0_i32..).zip(tokens_list.into_iter()) {
        // llama_decode will output logits only for the last token of the prompt
        let is_last = i == last_index;
        batch.add(token, i, &[0], is_last)?;
    }

    ctx.decode(&mut batch)
        .with_context(|| "llama_decode() failed")?;

    let mut n_cur = batch.n_tokens();

    // The `Decoder`
    let mut decoder = encoding_rs::UTF_8.new_decoder();

    let mut result = String::new();
    let mut output_string = String::with_capacity(32);
    let eos_token_id = config.model.token_eos();

    while n_cur <= config.n_len {
        let candidates = ctx.candidates_ith(batch.n_tokens() - 1);
        let mut candidates_p = LlamaTokenDataArray::from_iter(candidates, false);

        // sample the most likely token
        ctx.sample_tail_free(&mut candidates_p, 0.85, 1);
        let new_token_id = candidates_p
            .data
            .iter()
            .choose(&mut rand::thread_rng())
            .unwrap()
            .id();

        // consider https://docs.rs/weighted_rand/latest/weighted_rand/

        if new_token_id == eos_token_id {
            break;
        }

        let output_bytes = config
            .model
            .token_to_bytes(new_token_id, Special::Tokenize)?;

        let _decode_result = decoder.decode_to_string(&output_bytes, &mut output_string, false);
        result.push_str(&output_string);
        output_string.clear();

        batch.clear();
        batch.add(new_token_id, n_cur, &[0], true)?;

        n_cur += 1;

        ctx.decode(&mut batch).with_context(|| "failed to eval")?;
    }

    Ok(result
        .trim_start_matches("{")
        .trim_end_matches("}")
        .trim()
        .to_string())
}
