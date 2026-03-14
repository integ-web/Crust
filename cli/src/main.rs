use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use engine::InferenceEngine;
use env_logger::Env;
use graph_db::GraphDB;
use log::{info, warn};
use orchestrator::Orchestrator;
use scraper::ToolSynthesizer;
use std::io::{self, Write};
use sysinfo::System;
use serde::Deserialize;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a new research query
    Research {
        /// The initial query or topic
        #[arg(short, long)]
        query: Option<String>,

        /// Time limit in minutes
        #[arg(short, long, default_value_t = 30)]
        time_limit: u32,
    },
    /// Initialize system and download models (10-minute setup)
    Init,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    match &cli.command {
        Commands::Init => {
            info!("Running 10-Minute Setup...");
            setup_environment().await?;
        }
        Commands::Research { query, time_limit } => {
            let actual_query = match query {
                Some(q) => q.clone(),
                None => interactive_refine_query()?,
            };

            info!("Starting deep research on: {}", actual_query);
            info!("Time limit constraint: {} minutes", time_limit);

            // 1. Initialize dependencies
            let db_path = "research_memory.db";
            let graph = GraphDB::new(db_path).context("Failed to initialize Ontology DB")?;

            let scraper = ToolSynthesizer::new().context("Failed to initialize Scraper Tools")?;

            let engine = InferenceEngine::new("tinyllama.gguf", "tokenizer.json")
                .context("Failed to initialize Inference Engine. Did you run `cli init`?")?;

            let mut orchestrator = Orchestrator::new(engine, graph);

            let scraper_arc = std::sync::Arc::new(scraper);
            orchestrator.register_tool(Box::new(orchestrator::web_search_tool::WebSearchTool::new(scraper_arc.clone())));
            orchestrator.register_tool(Box::new(orchestrator::builtin_tools::ArxivTool::new(scraper_arc.clone())));
            orchestrator.register_tool(Box::new(orchestrator::builtin_tools::BrowseTool::new(scraper_arc.clone())));

            let result = orchestrator.execute_research(&actual_query).await?;
            info!("Final Research Result: \n{}", result);
        }
    }

    Ok(())
}

fn interactive_refine_query() -> Result<String> {
    let mut query = String::new();
    println!("=== Deep Research Agent Interactive Refining ===");
    println!("What topic would you like to research? (e.g., 'Find all YC startups doing AI dev tools and extract their leads')");
    print!("> ");
    io::stdout().flush()?;
    io::stdin().read_line(&mut query)?;

    // Stub for interactive refinement:
    // In a full implementation, the LLM would quickly evaluate the query here and ask
    // clarifying questions (like target geography, specific sub-niches) before starting.
    println!("Got it. I will optimize this for the best research path.");

    Ok(query.trim().to_string())
}

#[derive(Deserialize, Debug)]
struct HFModel {
    id: String,
}

async fn fetch_trending_gguf_models() -> Result<Vec<String>> {
    let url = "https://huggingface.co/api/models?search=GGUF&filter=text-generation&sort=downloads&direction=-1&limit=20";
    let client = reqwest::Client::builder().user_agent("RustyAgent/1.0").build()?;
    let response = client.get(url).send().await?.json::<Vec<HFModel>>().await?;
    Ok(response.into_iter().map(|m| m.id).collect())
}

fn determine_model_tier(repo_id: &str, total_ram_gb: f64) -> bool {
    let id_lower = repo_id.to_lowercase();

    // Extract rough parameter count from name
    let is_tiny = id_lower.contains("0.5b") || id_lower.contains("1b") || id_lower.contains("1.5b") || id_lower.contains("2b");
    let is_medium = id_lower.contains("3b") || id_lower.contains("4b");
    let _is_large = id_lower.contains("7b") || id_lower.contains("8b");

    // Fit to RAM constraint
    if total_ram_gb < 4.5 {
        is_tiny // Only allow sub-3B models on 4GB systems
    } else if total_ram_gb < 12.0 {
        is_tiny || is_medium // Allow up to 4B on 8GB systems
    } else {
        true // Allow anything on 12GB+ systems
    }
}

async fn setup_environment() -> Result<()> {
    println!("=== Hardware Discovery (LLMFit) ===");
    let mut sys = System::new_all();
    sys.refresh_all();

    let total_ram_gb = sys.total_memory() as f64 / 1_073_741_824.0;
    let cpu_cores = sys.cpus().len();

    println!("Detected Hardware:");
    println!("- RAM: {:.2} GB", total_ram_gb);
    println!("- CPU: {} logical cores", cpu_cores);

    println!("\n=== Dynamic Model Discovery ===");
    println!("Scraping HuggingFace for top trending GGUF models that fit your hardware...");

    let all_trending = fetch_trending_gguf_models().await.unwrap_or_else(|_| {
        warn!("Failed to fetch live models. Falling back to default.");
        vec!["TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF".to_string()]
    });

    let viable_models: Vec<String> = all_trending.into_iter()
        .filter(|id| determine_model_tier(id, total_ram_gb))
        .take(5)
        .collect();

    println!("\nFound {} optimal models for your {:.1} GB system:", viable_models.len(), total_ram_gb);
    for (i, model) in viable_models.iter().enumerate() {
        println!("{}. {}", i + 1, model);
    }

    println!("\nEnter the number of the model to download (or press [ENTER] for default #1):");
    print!("> ");
    io::stdout().flush()?;

    let mut choice = String::new();
    io::stdin().read_line(&mut choice)?;
    let choice = choice.trim();

    let selected_index = if choice.is_empty() {
        0
    } else {
        choice.parse::<usize>().unwrap_or(1).saturating_sub(1)
    };

    let selected_repo = viable_models.get(selected_index).unwrap_or(&viable_models[0]);
    println!("Selected: {}", selected_repo);

    info!("Initializing HuggingFace Hub API...");
    let api = hf_hub::api::tokio::Api::new()?;
    let model_repo = api.model(selected_repo.to_string());

    // We scrape the repo files to find the Q4_K_M or Q8_0 weights dynamically
    let info = model_repo.info().await?;
    let mut chosen_file = String::new();
    for file in info.siblings {
        let rfilename = file.rfilename;
        if rfilename.ends_with(".gguf") {
            let fname = rfilename.to_lowercase();
            // Prefer Q4_K_M for balance of speed and size
            if fname.contains("q4_k_m") {
                chosen_file = rfilename.clone();
                break;
            } else if fname.contains("q8_0") {
                chosen_file = rfilename.clone();
            } else if chosen_file.is_empty() {
                chosen_file = rfilename;
            }
        }
    }

    if chosen_file.is_empty() {
        anyhow::bail!("No .gguf files found in repository {}", selected_repo);
    }

    info!("Downloading optimal weight file ({})...", chosen_file);
    let model_path = model_repo.get(&chosen_file).await?;

    // Tokenizer is rarely in quantized repos. For an autonomous setup,
    // we would extract the base model name from config.json.
    // For this demonstration, we map a few popular base repos, otherwise fallback to TinyLlama base.
    let base_repo_name = if selected_repo.to_lowercase().contains("llama-3.2") {
        "meta-llama/Llama-3.2-3B-Instruct"
    } else if selected_repo.to_lowercase().contains("qwen") {
        "Qwen/Qwen2.5-3B-Instruct"
    } else if selected_repo.to_lowercase().contains("gemma") {
        "google/gemma-2-2b-it"
    } else {
        "TinyLlama/TinyLlama-1.1B-Chat-v1.0"
    };

    info!("Attempting to download tokenizer from base repo: {}", base_repo_name);
    let base_repo = api.model(base_repo_name.to_string());
    let tokenizer_path = base_repo.get("tokenizer.json").await?;

    std::fs::copy(&model_path, "model.gguf")
        .unwrap_or_else(|_| { warn!("Failed to copy model to local dir, using cache directly."); 0 });
    std::fs::copy(&tokenizer_path, "tokenizer.json")
        .unwrap_or_else(|_| { warn!("Failed to copy tokenizer to local dir, using cache directly."); 0 });

    info!("Setup complete! The dynamically selected LLM has been installed. You are ready to run `cli research`.");
    Ok(())
}
