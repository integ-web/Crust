use anyhow::{Context, Result};
use candle_core::{Device, Tensor};
use candle_core::quantized::gguf_file;
use candle_transformers::models::quantized_llama::ModelWeights;
use candle_transformers::generation::LogitsProcessor;
use log::info;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokenizers::Tokenizer;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

pub struct InferenceEngine {
    device: Device,
    tokenizer: Tokenizer,
    model: ModelWeights,
}

impl InferenceEngine {
    pub fn new(model_path: &str, tokenizer_path: &str) -> Result<Self> {
        info!("Initializing Inference Engine...");

        let device = Device::Cpu; // Constrained hardware relies on CPU primarily.

        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        let model_path = PathBuf::from(model_path);

        // This parses the GGUF file and loads the quantized weights directly into memory
        let mut file = std::fs::File::open(&model_path)?;
        let gguf_content = gguf_file::Content::read(&mut file)
            .context("Failed to read GGUF file content")?;
        let model = ModelWeights::from_gguf(gguf_content, &mut file, &device)?;

        Ok(Self {
            device,
            tokenizer,
            model,
        })
    }

    /// Formats the conversation history into the specific ChatML template required by the model.
    /// This forces the model to actually "remember" the context of its previous tool uses.
    fn format_chatml(&self, history: &[Message]) -> String {
        let mut prompt = String::new();
        for msg in history {
            let role_str = match msg.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::Tool => "tool", // Often handled by system or user if the model doesn't natively support tool roles
            };
            prompt.push_str(&format!("<|{}|>\n{}</s>\n", role_str, msg.content));
        }
        // Append the final assistant prompt so it knows it is its turn to speak
        prompt.push_str("<|assistant|>\n");
        prompt
    }

    /// Generates text by passing a full conversation context (memory) to the model.
    pub fn generate(&mut self, history: &[Message], max_tokens: usize) -> Result<String> {
        let chat_prompt = self.format_chatml(history);
        info!("Generating response for context length: {} chars", chat_prompt.len());

        let tokens = self.tokenizer
            .encode(chat_prompt, true)
            .map_err(|e| anyhow::anyhow!("Tokenization error: {}", e))?;

        let mut tokens_ids = tokens.get_ids().to_vec();

        // Add temperature (0.1) for deterministic output, and a repetition penalty to prevent loops
        let mut logits_processor = LogitsProcessor::new(299792458, Some(0.1), None);

        let mut generated_text = String::new();
        let eos_token = self.tokenizer.get_vocab(true).get("</s>").copied();

        info!("Running inference... (this will take a moment depending on your CPU)");

        let repeat_penalty: f32 = 1.2;
        let repeat_last_n = 64;

        for index in 0..max_tokens {
            let context_size = if index > 0 { 1 } else { tokens_ids.len() };
            let start_pos = tokens_ids.len().saturating_sub(context_size);

            let input_tensor = Tensor::new(&tokens_ids[start_pos..], &self.device)?.unsqueeze(0)?;

            let logits = self.model.forward(&input_tensor, start_pos)?;
            let mut logits = logits.squeeze(0)?.squeeze(0)?;

            // Apply repetition penalty to the last N tokens
            if repeat_penalty != 1.0 && !tokens_ids.is_empty() {
                let start_idx = tokens_ids.len().saturating_sub(repeat_last_n);
                let ctx = &tokens_ids[start_idx..];
                logits = candle_transformers::utils::apply_repeat_penalty(&logits, repeat_penalty, ctx)?;
            }

            let next_token = logits_processor.sample(&logits)?;

            if Some(next_token) == eos_token {
                break;
            }

            tokens_ids.push(next_token);
        }

        // Decode all at once to preserve spaces properly
        if let Some(text) = self.tokenizer.decode(&tokens_ids[tokens.get_ids().len()..], true).ok() {
            generated_text = text;
        }

        Ok(generated_text)
    }
}
