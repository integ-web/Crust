use anyhow::Result;
use brain::taint::{PrincipalChecker, UntrustedValue, TrustedAction};
use interface::dispatcher::{DesignDispatcher, OutputFormat};
use interface::synthesis::SynthesisEngine;
use kernel::prober::HardwareProber;
use memory::episodic::EpisodicMemory;
use memory::semantic::SemanticMemory;
use orchestrator::orga::OrgaCycle;
use senses::browser::StealthBrowser;
use std::time::Duration;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // 0. Initialize System Tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Setting default subscriber failed");

    info!("🚀 Booting Crust-RustyAgent OS Kernel...");

    // 1. Session 1: Hardware-Aware Matchmaker & Bootloader
    let prober = HardwareProber::new();
    let hw_profile = prober.probe();
    info!("Hardware Profile Detected: {:?}", hw_profile);
    info!("Estimated Kernel Speed: {:.2} TPS", hw_profile.speed_estimate_tps);

    // 2. Session 4 & 5: Initialize Memory and Durable Event Log
    let semantic_db = SemanticMemory::new()?;
    let episodic_log = EpisodicMemory::new("agent_episodic.log");
    info!("Multi-Tier Memory and Event Logs Initialized.");

    // 3. User Input (Sample Research Query)
    let query = "Personal finance manager for startup founders";
    info!("User Prompt Recieved: '{}'", query);

    // 4. Session 6: Core Orchestration (ORGA via DAG)
    let mut orga = OrgaCycle::new();

    // In a real run, the ORGA cycle triggers `senses` (StealthBrowser) based on its generated sub-tasks.
    // For demonstration of the OS integrating the flow, we will manually perform the pipeline:

    // -> Start the true DAG-based ORGA reasoning cycle
    orga.run_dag_cycle(query).await?;

    // -> Sense / Scrape (Mocking Adaptive Senses - Session 3)
    info!("Senses: Launching Stealth Browser to research personal finance tools...");
    let browser_sim = async {
        let browser = StealthBrowser::launch().await?;
        // Normally we navigate here: let page = browser.stealth_page("https://example-startup-finance.com").await?;
        // browser.human_mouse_move(&page, 0.0, 0.0, 100.0, 100.0).await?;
        Ok::<&str, anyhow::Error>("Found 'Mint for Startups' and 'Brex'.")
    };

    let raw_findings = match browser_sim.await {
        Ok(data) => data.to_string(),
        Err(e) => {
            tracing::warn!("Browser failed, falling back to cached knowledge: {}", e);
            "Fallback data: Founder-focused bank accounts with runway calculators.".to_string()
        }
    };

    // 5. Session 2: Taint Analysis & Security Gating
    info!("Security: Routing scraped data through Principal Checker.");
    let untrusted_findings = UntrustedValue::new(raw_findings, "web_search".to_string());

    // Policy P-T: Must not contain malware indicators
    let policy = |data: &String| !data.contains("malware");
    let trusted_findings = PrincipalChecker::sanitize(untrusted_findings, policy)
        .map_err(|e| anyhow::anyhow!("Security Exception: {}", e))?;

    // 6. Memorization
    info!("Memory: Reifying knowledge into RDF Graph and Episodic Merkle Log.");
    let safe_data = trusted_findings.into_inner();

    // Hash chain the event
    let block = episodic_log.append_event(&format!("SCRAPED: {}", safe_data))?;
    info!("Appended Episodic Block: {}", block.block_hash);

    // Reify to Semantic Store (Subject, Predicate, Object)
    // URIs must have a scheme (e.g. ex: or http://) for Oxigraph/RDF compliance
    semantic_db.insert_triplet("http://example.org/startup_finance", "http://example.org/includes_tool", "Brex")?;
    semantic_db.insert_triplet("http://example.org/startup_finance", "http://example.org/requires_feature", "Runway Calculator")?;

    // 7. Session 7: Multi-Format Synthesis & Design Engine
    let sparql_query = "SELECT ?s ?p ?o WHERE { ?s ?p ?o }";
    let retrieved_knowledge = semantic_db.query_sparql(sparql_query)?;

    let json_ld_data = SynthesisEngine::extract_to_jsonld(&retrieved_knowledge)?;

    let dashboard_html = DesignDispatcher::dispatch_design(
        &json_ld_data,
        OutputFormat::HtmlTailwindDashboard,
        "Modern McKinsey Report Style"
    )?;

    info!("=== OS EXECUTION COMPLETE ===");
    println!("\n[Final Output HTML Artifact]\n{}", dashboard_html);

    Ok(())
}
