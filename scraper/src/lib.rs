use anyhow::{Context, Result};
use chromiumoxide::browser::{Browser, BrowserConfig};
use log::info;
use std::time::Duration;
use tokio_retry::Retry;
use tokio_retry::strategy::ExponentialBackoff;
use futures_util::stream::StreamExt;

pub struct ToolSynthesizer {
    // Used to spawn the chromiumoxide runtime
}

impl ToolSynthesizer {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }

    /// Executes headless browser automation to render JS and extract text, bypassing basic protections.
    /// Runs with aggressive memory-saving flags to adhere to the 4GB environment limit.
    pub async fn browse_and_extract(&self, url: &str) -> Result<String> {
        info!("Headless Browsing: {}", url);

        let config = BrowserConfig::builder()
            // We do NOT use .with_head() because we strictly want headless for 4GB RAM edge devices
            .no_sandbox()
            .arg("--disable-gpu")
            .arg("--disable-dev-shm-usage")
            .arg("--blink-settings=imagesEnabled=false") // Aggressive memory saving: don't load images
            .arg("--disable-extensions")
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build browser config: {:?}", e))?;

        let (mut browser, mut handler) = Browser::launch(config).await?;

        // Spawn a background task that handles browser events (required by chromiumoxide)
        let handle = tokio::task::spawn(async move {
            while let Some(h) = handler.next().await {
                if let Err(_) = h {
                    break;
                }
            }
        });

        // 1. Navigate to page
        let page = browser.new_page(url).await?;

        // 2. Wait for rendering (Network idle)
        page.wait_for_navigation().await?;
        tokio::time::sleep(Duration::from_secs(2)).await; // Hard wait for JS SPAs

        // 3. Extract the innerText of the body (this skips hidden elements, script tags, and CSS automatically!)
        let js_evaluation = page.evaluate("document.body.innerText").await?;

        let extracted_text = js_evaluation.value().map(|v| v.as_str().unwrap_or("").to_string()).unwrap_or_default();

        info!("Successfully extracted {} bytes of rendered text from {}", extracted_text.len(), url);

        // Cleanup
        browser.close().await?;
        let _ = handle.await;

        Ok(extracted_text)
    }

    /// Real Web Search Tool (DuckDuckGo HTML Scraping)
    /// Returns a list of URLs and snippets
    pub async fn search_web(&self, query: &str) -> Result<String> {
        info!("Web Searching: {}", query);

        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()?;

        let url = format!("https://html.duckduckgo.com/html/?q={}", query.replace(" ", "+"));

        let retry_strategy = ExponentialBackoff::from_millis(500).take(2);

        let html = Retry::spawn(retry_strategy, || async {
            let res = client.get(&url).send().await?;
            if !res.status().is_success() {
                return Err(anyhow::anyhow!("DDG Search error: {}", res.status()));
            }
            res.text().await.map_err(|e| anyhow::anyhow!(e))
        }).await.context("Failed to fetch search results")?;

        // Parse HTML to extract actual search results using `select` crate
        use select::document::Document;
        use select::predicate::Class;

        let document = Document::from(html.as_str());
        let mut results = String::new();

        for node in document.find(Class("result")) {
            if let Some(a_tag) = node.find(Class("result__a")).next() {
                let title = a_tag.text();
                let link = a_tag.attr("href").unwrap_or("");

                // DDG wraps links, we need to extract the actual URL
                let clean_link = if link.starts_with("//duckduckgo.com/l/?uddg=") {
                    let decoded = urlencoding::decode(link.trim_start_matches("//duckduckgo.com/l/?uddg=").split('&').next().unwrap_or("")).unwrap_or_default();
                    decoded.into_owned()
                } else {
                    link.to_string()
                };

                let snippet = node.find(Class("result__snippet")).next().map(|n| n.text()).unwrap_or_default();

                results.push_str(&format!("Title: {}\nURL: {}\nSnippet: {}\n\n", title.trim(), clean_link, snippet.trim()));
            }
        }

        if results.is_empty() {
            Ok("No search results found. The engine might be blocking the request.".to_string())
        } else {
            Ok(results)
        }
    }

    /// Academic Data Fetcher using reqwest directly (since it's a raw XML API, no headless browser needed)
    pub async fn search_arxiv(&self, query: &str) -> Result<String> {
        info!("Searching Arxiv for: {}", query);
        let url = format!("http://export.arxiv.org/api/query?search_query=all:{}&start=0&max_results=3", query);

        let client = reqwest::Client::new();
        let retry_strategy = ExponentialBackoff::from_millis(200).take(3);

        let xml_text = Retry::spawn(retry_strategy, || async {
            let res = client.get(&url).send().await?;
            if !res.status().is_success() {
                return Err(anyhow::anyhow!("Arxiv API error: {}", res.status()));
            }
            res.text().await.map_err(|e| anyhow::anyhow!(e))
        }).await.context("Failed to fetch Arxiv data after retries")?;

        Ok(xml_text)
    }
}
