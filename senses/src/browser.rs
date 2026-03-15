use anyhow::Result;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::page::Page;
use chromiumoxide::layout::Point;
use futures::StreamExt;
use std::time::Duration;
use tracing::info;

pub struct StealthBrowser {
    browser: Browser,
}

impl StealthBrowser {
    pub async fn launch() -> Result<Self> {
        let (browser, mut handler) = Browser::launch(
            BrowserConfig::builder()
                .with_head()
                .build()
                .map_err(|e| anyhow::anyhow!("Failed to build config: {}", e))?
        )
        .await?;

        // Spawn a background task to handle browser events
        tokio::task::spawn(async move {
            while let Some(h) = handler.next().await {
                if h.is_err() {
                    break;
                }
            }
        });

        Ok(Self { browser })
    }

    pub async fn stealth_page(&self, url: &str) -> Result<Page> {
        let page = self.browser.new_page(url).await?;

        // Stealth Pillar 7: Inject bootstrap script to mock WebGL and navigator.webdriver
        let stealth_script = r#"
            Object.defineProperty(navigator, 'webdriver', {
                get: () => false,
            });
            const getParameterProxyHandler = {
                apply: function (target, ctx, args) {
                    const param = (args || [])[0];
                    if (param === 37445) return 'Google Inc. (Apple)'; // UNMASKED_VENDOR_WEBGL
                    if (param === 37446) return 'Apple GPU'; // UNMASKED_RENDERER_WEBGL
                    return Reflect.apply(target, ctx, args);
                }
            };
            const proxy = new Proxy(WebGLRenderingContext.prototype.getParameter, getParameterProxyHandler);
            Object.defineProperty(WebGLRenderingContext.prototype, 'getParameter', {
                configurable: true,
                enumerable: true,
                writable: true,
                value: proxy
            });
        "#;

        page.evaluate(stealth_script).await?;
        info!("Stealth script injected into page: {}", url);

        Ok(page)
    }

    /// Simulates physical human interaction via randomized Bezier curve mouse movement.
    pub async fn human_mouse_move(&self, page: &Page, start_x: f64, start_y: f64, end_x: f64, end_y: f64) -> Result<()> {
        let steps = 15;
        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            // A simple cubic bezier curve approximation for jitter
            let jitter = (t * (1.0 - t)) * 20.0;

            let x = start_x + (end_x - start_x) * t + jitter;
            let y = start_y + (end_y - start_y) * t + jitter;

            let point = Point { x, y };
            page.move_mouse(point).await?;
            tokio::time::sleep(Duration::from_millis(10 + (fastrand::u64(0..15) as u64))).await;
        }
        Ok(())
    }
}
