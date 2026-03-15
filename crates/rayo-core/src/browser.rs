//! Browser and Page wrappers around chromiumoxide.
//!
//! Adds rayo's AI-native features on top: page maps, batch execution,
//! selector caching, profiling.

use std::sync::Arc;

use chromiumoxide::browser::{Browser as CdpBrowser, BrowserConfig};
use chromiumoxide::page::Page as CdpPage;
use chromiumoxide::cdp::browser_protocol::page::{CaptureScreenshotFormat, CaptureScreenshotParams};
use futures::StreamExt;
use tokio::sync::Mutex;

use crate::batch::{BatchAction, BatchActionResult, BatchResult, ActionTarget};
use crate::error::RayoError;
use crate::page_map::{PageMap, EXTRACT_PAGE_MAP_JS};
use crate::selector_cache::SelectorCache;
use rayo_profiler::{Profiler, SpanCategory};

/// Rayo browser wrapper with profiling and caching.
pub struct RayoBrowser {
    browser: CdpBrowser,
    _handler_handle: tokio::task::JoinHandle<()>,
    pub profiler: Profiler,
}

impl RayoBrowser {
    /// Launch a new headless Chrome instance.
    pub async fn launch() -> Result<Self, RayoError> {
        let config = BrowserConfig::builder()
            .no_sandbox()
            .arg("--disable-gpu")
            .arg("--disable-dev-shm-usage")
            .build()
            .map_err(|e| RayoError::Cdp(format!("Failed to build browser config: {e}")))?;

        let (browser, mut handler) = CdpBrowser::launch(config)
            .await
            .map_err(|e| RayoError::Cdp(format!("Failed to launch browser: {e}")))?;

        let handle = tokio::spawn(async move {
            while handler.next().await.is_some() {}
        });

        Ok(Self {
            browser,
            _handler_handle: handle,
            profiler: Profiler::new(),
        })
    }

    /// Connect to an already-running Chrome instance.
    pub async fn connect(url: &str) -> Result<Self, RayoError> {
        let (browser, mut handler) = CdpBrowser::connect(url)
            .await
            .map_err(|e| RayoError::Cdp(format!("Failed to connect to browser: {e}")))?;

        let handle = tokio::spawn(async move {
            while handler.next().await.is_some() {}
        });

        Ok(Self {
            browser,
            _handler_handle: handle,
            profiler: Profiler::new(),
        })
    }

    /// Create a new page (tab).
    pub async fn new_page(&self) -> Result<RayoPage, RayoError> {
        let _span = self.profiler.start_span("new_page", SpanCategory::Navigation);
        let page = self.browser.new_page("about:blank").await?;
        Ok(RayoPage {
            page,
            selector_cache: Arc::new(Mutex::new(SelectorCache::new(1024))),
            profiler: self.profiler.clone(),
            page_map_cache: Arc::new(Mutex::new(None)),
        })
    }

    /// Get the profiler.
    pub fn profiler(&self) -> &Profiler {
        &self.profiler
    }
}

/// Rayo page wrapper with AI-native features.
pub struct RayoPage {
    page: CdpPage,
    selector_cache: Arc<Mutex<SelectorCache>>,
    profiler: Profiler,
    page_map_cache: Arc<Mutex<Option<PageMap>>>,
}

impl RayoPage {
    /// Navigate to a URL.
    pub async fn goto(&self, url: &str) -> Result<(), RayoError> {
        let _span = self.profiler.start_span(
            format!("goto({})", truncate(url, 60)),
            SpanCategory::Navigation,
        );
        self.page.goto(url).await?;
        // Invalidate caches on navigation
        self.selector_cache.lock().await.invalidate();
        *self.page_map_cache.lock().await = None;
        Ok(())
    }

    /// Reload the page.
    pub async fn reload(&self) -> Result<(), RayoError> {
        let _span = self.profiler.start_span("reload", SpanCategory::Navigation);
        self.page.reload().await?;
        self.selector_cache.lock().await.invalidate();
        *self.page_map_cache.lock().await = None;
        Ok(())
    }

    /// Get the current URL.
    pub async fn url(&self) -> Result<String, RayoError> {
        let result = self.page.evaluate("window.location.href").await?;
        Ok(result.into_value::<String>().unwrap_or_default())
    }

    /// Get the page title.
    pub async fn title(&self) -> Result<String, RayoError> {
        let result = self.page.evaluate("document.title").await?;
        Ok(result.into_value::<String>().unwrap_or_default())
    }

    /// Generate a token-efficient page map for LLMs (~500 tokens).
    pub async fn page_map(&self) -> Result<PageMap, RayoError> {
        let _span = self.profiler.start_span("page_map", SpanCategory::PageMap);
        let result = self.page.evaluate(EXTRACT_PAGE_MAP_JS).await?;
        let map: PageMap = result.into_value().map_err(|e| {
            RayoError::Cdp(format!("Failed to deserialize page map: {e:?}"))
        })?;
        *self.page_map_cache.lock().await = Some(map.clone());
        Ok(map)
    }

    /// Get text content of the page or a specific element.
    pub async fn text_content(&self, selector: Option<&str>) -> Result<String, RayoError> {
        let _span = self.profiler.start_span("text_content", SpanCategory::DomRead);
        let js = match selector {
            Some(sel) => format!(
                "document.querySelector({}).textContent",
                serde_json::to_string(sel).unwrap()
            ),
            None => "document.body.innerText".to_string(),
        };
        let result = self.page.evaluate(js).await?;
        Ok(result.into_value::<String>().unwrap_or_default())
    }

    /// Take a screenshot, returns base64-encoded PNG.
    pub async fn screenshot(&self, _full_page: bool) -> Result<String, RayoError> {
        let _span = self.profiler.start_span("screenshot", SpanCategory::Screenshot);
        let mut params = CaptureScreenshotParams::default();
        params.format = Some(CaptureScreenshotFormat::Png);
        let bytes = self.page.screenshot(params).await?;
        use base64::Engine;
        Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
    }

    /// Click an element by selector or page map ID.
    pub async fn click(&self, selector: Option<&str>, id: Option<usize>) -> Result<(), RayoError> {
        let sel = self.resolve_selector(selector, id).await?;
        let _span = self.profiler.start_span(
            format!("click({})", truncate(&sel, 40)),
            SpanCategory::DomMutate,
        );
        let js = format!(
            r#"(() => {{
                const el = document.querySelector({});
                if (!el) throw new Error('Element not found: {}');
                el.scrollIntoView({{block: 'center'}});
                el.click();
                return true;
            }})()"#,
            serde_json::to_string(&sel).unwrap(),
            sel.replace('\'', "\\'"),
        );
        self.page.evaluate(js).await?;
        self.selector_cache.lock().await.invalidate();
        *self.page_map_cache.lock().await = None;
        Ok(())
    }

    /// Type text into an element.
    pub async fn type_text(
        &self,
        selector: Option<&str>,
        id: Option<usize>,
        text: &str,
        clear: bool,
    ) -> Result<(), RayoError> {
        let sel = self.resolve_selector(selector, id).await?;
        let _span = self.profiler.start_span(
            format!("type({})", truncate(&sel, 40)),
            SpanCategory::DomMutate,
        );
        let clear_js = if clear { "el.value = '';" } else { "" };
        let js = format!(
            r#"(() => {{
                const el = document.querySelector({sel_json});
                if (!el) throw new Error('Element not found');
                el.focus();
                {clear_js}
                el.value = {text_json};
                el.dispatchEvent(new Event('input', {{bubbles: true}}));
                el.dispatchEvent(new Event('change', {{bubbles: true}}));
                return true;
            }})()"#,
            sel_json = serde_json::to_string(&sel).unwrap(),
            text_json = serde_json::to_string(text).unwrap(),
        );
        self.page.evaluate(js).await?;
        Ok(())
    }

    /// Select an option from a dropdown.
    pub async fn select_option(
        &self,
        selector: Option<&str>,
        id: Option<usize>,
        value: &str,
    ) -> Result<(), RayoError> {
        let sel = self.resolve_selector(selector, id).await?;
        let _span = self.profiler.start_span("select", SpanCategory::DomMutate);
        let js = format!(
            r#"(() => {{
                const el = document.querySelector({sel_json});
                if (!el) throw new Error('Element not found');
                const opt = Array.from(el.options).find(o => o.value === {val_json} || o.text === {val_json});
                if (opt) {{ el.value = opt.value; el.dispatchEvent(new Event('change', {{bubbles: true}})); }}
                return true;
            }})()"#,
            sel_json = serde_json::to_string(&sel).unwrap(),
            val_json = serde_json::to_string(value).unwrap(),
        );
        self.page.evaluate(js).await?;
        Ok(())
    }

    /// Execute a batch of actions.
    pub async fn execute_batch(&self, actions: Vec<BatchAction>) -> Result<BatchResult, RayoError> {
        let _span = self.profiler.start_span(
            format!("batch({})", actions.len()),
            SpanCategory::Batch,
        );
        let start = std::time::Instant::now();
        let mut results = Vec::with_capacity(actions.len());
        let mut succeeded = 0usize;
        let mut failed = 0usize;

        for (i, action) in actions.iter().enumerate() {
            let action_start = std::time::Instant::now();
            let res = match action {
                BatchAction::Click { target } => {
                    let (sel, id) = target_to_selector_id(target);
                    self.click(sel, id).await.map(|_| None)
                }
                BatchAction::Type { target, value } => {
                    let (sel, id) = target_to_selector_id(target);
                    self.type_text(sel, id, value, true).await.map(|_| None)
                }
                BatchAction::Select { target, value } => {
                    let (sel, id) = target_to_selector_id(target);
                    self.select_option(sel, id, value).await.map(|_| None)
                }
                BatchAction::Goto { url } => {
                    self.goto(url).await.map(|_| None)
                }
                BatchAction::Screenshot { full_page } => {
                    self.screenshot(*full_page).await.map(|b64| Some(serde_json::Value::String(b64)))
                }
                BatchAction::WaitFor { target, timeout_ms } => {
                    let (sel, id) = target_to_selector_id(target);
                    let selector = self.resolve_selector(sel, id).await?;
                    self.wait_for_selector(&selector, *timeout_ms).await.map(|_| None)
                }
                BatchAction::Scroll { target, x, y } => {
                    if let Some(t) = target {
                        let (sel, id) = target_to_selector_id(t);
                        let selector = self.resolve_selector(sel, id).await?;
                        let js = format!(
                            "document.querySelector({}).scrollIntoView({{block:'center'}})",
                            serde_json::to_string(&selector).unwrap(),
                        );
                        self.page.evaluate(js).await.map(|_| None).map_err(RayoError::from)
                    } else {
                        let js = format!("window.scrollTo({x},{y})");
                        self.page.evaluate(js).await.map(|_| None).map_err(RayoError::from)
                    }
                }
            };

            let duration_ms = action_start.elapsed().as_secs_f64() * 1000.0;
            match res {
                Ok(data) => {
                    succeeded += 1;
                    results.push(BatchActionResult {
                        index: i,
                        action: action_name(action).to_string(),
                        success: true,
                        error: None,
                        data,
                        duration_ms,
                    });
                }
                Err(e) => {
                    failed += 1;
                    results.push(BatchActionResult {
                        index: i,
                        action: action_name(action).to_string(),
                        success: false,
                        error: Some(e.to_string()),
                        data: None,
                        duration_ms,
                    });
                }
            }
        }

        Ok(BatchResult {
            results,
            total_duration_ms: start.elapsed().as_secs_f64() * 1000.0,
            succeeded,
            failed,
        })
    }

    /// Wait for a selector to appear.
    pub async fn wait_for_selector(&self, selector: &str, timeout_ms: u64) -> Result<(), RayoError> {
        let _span = self.profiler.start_span(
            format!("wait({})", truncate(selector, 40)),
            SpanCategory::Wait,
        );
        let timeout = std::time::Duration::from_millis(timeout_ms);
        let start = std::time::Instant::now();
        let poll_interval = std::time::Duration::from_millis(50);

        loop {
            let js = format!(
                "!!document.querySelector({})",
                serde_json::to_string(selector).unwrap(),
            );
            let result = self.page.evaluate(js).await?;
            if result.into_value::<bool>().unwrap_or(false) {
                return Ok(());
            }
            if start.elapsed() > timeout {
                return Err(RayoError::Timeout {
                    what: selector.to_string(),
                    ms: timeout_ms,
                });
            }
            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Evaluate JavaScript on the page.
    pub async fn evaluate(&self, js: &str) -> Result<serde_json::Value, RayoError> {
        let _span = self.profiler.start_span("evaluate", SpanCategory::CdpCommand);
        let result = self.page.evaluate(js).await?;
        Ok(result.into_value().unwrap_or(serde_json::Value::Null))
    }

    /// Resolve a selector from either a CSS selector or a page map element ID.
    async fn resolve_selector(
        &self,
        selector: Option<&str>,
        id: Option<usize>,
    ) -> Result<String, RayoError> {
        if let Some(sel) = selector {
            return Ok(sel.to_string());
        }
        if let Some(element_id) = id {
            // Look up from cached page map
            let cache = self.page_map_cache.lock().await;
            if let Some(map) = cache.as_ref() {
                if let Some(el) = map.interactive.iter().find(|e| e.id == element_id) {
                    return Ok(el.selector.clone());
                }
            }
            drop(cache);
            // Refresh page map and retry
            let map = self.page_map().await?;
            if let Some(el) = map.interactive.iter().find(|e| e.id == element_id) {
                return Ok(el.selector.clone());
            }
            return Err(RayoError::ElementNotFound {
                selector: format!("page_map id={element_id}"),
            });
        }
        Err(RayoError::ElementNotFound {
            selector: "no selector or id provided".into(),
        })
    }
}

fn target_to_selector_id(target: &ActionTarget) -> (Option<&str>, Option<usize>) {
    match target {
        ActionTarget::Id { id } => (None, Some(*id)),
        ActionTarget::Selector { selector } => (Some(selector.as_str()), None),
    }
}

fn action_name(action: &BatchAction) -> &'static str {
    match action {
        BatchAction::Click { .. } => "click",
        BatchAction::Type { .. } => "type",
        BatchAction::Select { .. } => "select",
        BatchAction::Goto { .. } => "goto",
        BatchAction::Screenshot { .. } => "screenshot",
        BatchAction::WaitFor { .. } => "wait_for",
        BatchAction::Scroll { .. } => "scroll",
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() > max {
        &s[..max]
    } else {
        s
    }
}
