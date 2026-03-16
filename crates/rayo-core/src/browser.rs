//! Browser and Page wrappers around chromiumoxide.
//!
//! Adds rayo's AI-native features on top: page maps, batch execution,
//! selector caching, profiling.

use std::sync::Arc;

use crate::cookie::{CookieInfo, SameSite, SetCookie};
use crate::network::{CapturedRequest, NetworkInterceptor};
use chromiumoxide::browser::{Browser as CdpBrowser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::fetch::{
    ContinueRequestParams, EnableParams as FetchEnableParams, EventRequestPaused,
    FailRequestParams, FulfillRequestParams, HeaderEntry as FetchHeaderEntry,
};
use chromiumoxide::cdp::browser_protocol::network::{
    ClearBrowserCookiesParams, CookieParam, CookieSameSite, DeleteCookiesParams,
    EnableParams as NetworkEnableParams, ErrorReason, EventRequestWillBeSent,
    EventResponseReceived, TimeSinceEpoch,
};
use chromiumoxide::cdp::browser_protocol::page::{
    CaptureScreenshotFormat, CaptureScreenshotParams, Viewport,
};
use chromiumoxide::page::Page as CdpPage;
use futures::StreamExt;
use tokio::sync::Mutex;

use crate::batch::{ActionTarget, BatchAction, BatchActionResult, BatchResult};
use crate::error::RayoError;
use crate::page_map::{EXTRACT_PAGE_MAP_JS, PageMap};
use crate::selector_cache::SelectorCache;
use rayo_profiler::{Profiler, SpanCategory};

/// Rayo browser wrapper with profiling and caching.
pub struct RayoBrowser {
    browser: CdpBrowser,
    handler_task: tokio::task::JoinHandle<()>,
    pub profiler: Profiler,
    _user_data_dir: tempfile::TempDir,
}

impl RayoBrowser {
    /// Launch with a shared profiler (for MCP server integration).
    pub async fn launch_with_profiler(profiler: Profiler) -> Result<Self, RayoError> {
        let mut browser = Self::launch().await?;
        browser.profiler = profiler;
        Ok(browser)
    }

    /// Launch a new headless Chrome instance.
    pub async fn launch() -> Result<Self, RayoError> {
        let user_data_dir = tempfile::tempdir()
            .map_err(|e| RayoError::Cdp(format!("Failed to create temp dir: {e}")))?;

        let mut builder = BrowserConfig::builder()
            .arg("--disable-gpu")
            .arg("--disable-dev-shm-usage")
            .arg("--disable-extensions")
            .arg("--disable-background-networking")
            .arg("--disable-sync")
            .arg("--metrics-recording-only")
            .arg("--no-first-run")
            .arg("--disable-background-timer-throttling")
            .arg("--disable-default-apps")
            .window_size(1280, 720)
            .user_data_dir(user_data_dir.path());

        // Only disable sandbox in CI/containers where it causes launch failures
        if Self::should_no_sandbox() {
            builder = builder.no_sandbox();
        }

        // Use chrome-headless-shell for faster screenshots if available
        if let Some(path) = Self::find_chrome_executable() {
            eprintln!("[rayo] using chrome: {}", path.display());
            builder = builder.chrome_executable(path);
        }

        let config = builder
            .build()
            .map_err(|e| RayoError::Cdp(format!("Failed to build browser config: {e}")))?;

        let (browser, mut handler) = CdpBrowser::launch(config)
            .await
            .map_err(|e| RayoError::Cdp(format!("Failed to launch browser: {e}")))?;

        let handle = tokio::spawn(async move { while handler.next().await.is_some() {} });

        Ok(Self {
            browser,
            handler_task: handle,
            profiler: Profiler::new(),
            _user_data_dir: user_data_dir,
        })
    }

    /// Find the fastest available Chrome binary.
    /// Prefers chrome-headless-shell (purpose-built for headless, 10-30x faster screenshots).
    fn find_chrome_executable() -> Option<std::path::PathBuf> {
        // 1. User override
        if let Ok(path) = std::env::var("RAYO_CHROME_PATH") {
            let p = std::path::PathBuf::from(path);
            if p.exists() {
                return Some(p);
            }
        }

        // 2. Playwright's chrome-headless-shell cache
        let home = std::env::var("HOME").unwrap_or_default();
        let cache_dirs = [
            format!("{home}/Library/Caches/ms-playwright"), // macOS
            format!("{home}/.cache/ms-playwright"),         // Linux
        ];
        for cache_dir in &cache_dirs {
            if let Ok(entries) = std::fs::read_dir(cache_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with("chromium_headless_shell") {
                        // Look for the binary inside
                        if let Ok(inner) = std::fs::read_dir(entry.path()) {
                            for inner_entry in inner.flatten() {
                                let bin = inner_entry.path().join("chrome-headless-shell");
                                if bin.exists() {
                                    return Some(bin);
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. On PATH
        if let Ok(output) = std::process::Command::new("which")
            .arg("chrome-headless-shell")
            .output()
            && output.status.success()
        {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(std::path::PathBuf::from(path));
            }
        }

        // 4. Fall back to chromiumoxide's default detection
        None
    }

    /// Detect if Chrome sandbox should be disabled.
    /// Disabled in CI, Docker, or when explicitly requested.
    fn should_no_sandbox() -> bool {
        std::env::var("CI").is_ok()
            || std::env::var("RAYO_NO_SANDBOX").is_ok()
            || std::path::Path::new("/.dockerenv").exists()
            || std::path::Path::new("/run/.containerenv").exists()
    }

    /// Connect to an already-running Chrome instance.
    pub async fn connect(url: &str) -> Result<Self, RayoError> {
        let user_data_dir = tempfile::tempdir()
            .map_err(|e| RayoError::Cdp(format!("Failed to create temp dir: {e}")))?;

        let (browser, mut handler) = CdpBrowser::connect(url)
            .await
            .map_err(|e| RayoError::Cdp(format!("Failed to connect to browser: {e}")))?;

        let handle = tokio::spawn(async move { while handler.next().await.is_some() {} });

        Ok(Self {
            browser,
            handler_task: handle,
            profiler: Profiler::new(),
            _user_data_dir: user_data_dir,
        })
    }

    /// Create a new page (tab).
    pub async fn new_page(&self) -> Result<RayoPage, RayoError> {
        let _span = self
            .profiler
            .start_span("new_page", SpanCategory::Navigation);
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

impl Drop for RayoBrowser {
    fn drop(&mut self) {
        self.handler_task.abort();
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
    /// Invalidate caches after a DOM mutation (click, type, select).
    /// Centralizes the invalidation policy so it's consistent across all mutation methods.
    async fn invalidate_after_mutation(&self) {
        self.selector_cache.lock().await.invalidate();
        *self.page_map_cache.lock().await = None;
    }

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
    ///
    /// When `selector` is `Some`, the page map is scoped to that subtree:
    /// interactive elements, headings, and text summary are all extracted
    /// from within the matched element only.
    pub async fn page_map(&self, selector: Option<&str>) -> Result<PageMap, RayoError> {
        let _span = self.profiler.start_span("page_map", SpanCategory::PageMap);
        let js = match selector {
            Some(sel) => {
                let sel_json = serde_json::to_string(sel).unwrap();
                format!(
                    r#"
(() => {{
    const root = document.querySelector({sel_json});
    if (!root) return {{
        url: window.location.href,
        title: document.title,
        interactive: [],
        headings: [],
        text_summary: "",
    }};

    const interactive = [];
    const selectors = 'a[href], button, input, select, textarea, [role="button"], [role="link"], [role="tab"], [onclick]';
    const elements = root.querySelectorAll(selectors);

    const MAX_ELEMENTS = 50;
    let count = 0;
    elements.forEach((el, idx) => {{
        if (count >= MAX_ELEMENTS) return;
        if (el.offsetParent === null && el.type !== 'hidden') return;

        const item = {{ id: idx, tag: el.tagName.toLowerCase(), selector: '' }};

        if (el.type) item.type = el.type;
        if (el.name) item.name = el.name;

        const labelEl = el.labels && el.labels[0];
        if (labelEl) {{
            item.label = labelEl.textContent.trim();
        }} else if (el.getAttribute('aria-label')) {{
            item.label = el.getAttribute('aria-label');
        }} else if (el.placeholder) {{
            item.label = el.placeholder;
        }}

        const text = el.textContent?.trim();
        if (text && text.length < 100 && (el.tagName === 'BUTTON' || el.tagName === 'A')) {{
            item.text = text;
        }}

        if (el.placeholder) item.placeholder = el.placeholder;
        if (el.value && el.type !== 'password') item.value = el.value;

        if (el.tagName === 'SELECT') {{
            item.options = Array.from(el.options).map(o => o.text || o.value);
        }}

        if (el.type === 'radio' || el.type === 'checkbox') {{
            const group = document.querySelectorAll(`input[name="${{el.name}}"]`);
            if (group.length > 1) {{
                item.options = Array.from(group).map(r => r.value);
            }}
        }}

        const role = el.getAttribute('role');
        if (role) item.role = role;
        if (el.href) item.href = el.href.length > 120 ? el.href.slice(0, 120) : el.href;

        if (el.id) {{
            item.selector = '#' + CSS.escape(el.id);
        }} else if (el.name) {{
            item.selector = `${{el.tagName.toLowerCase()}}[name="${{el.name}}"]`;
        }} else {{
            const parent = el.parentElement;
            if (parent) {{
                const siblings = parent.querySelectorAll(':scope > ' + el.tagName.toLowerCase());
                const index = Array.from(siblings).indexOf(el) + 1;
                item.selector = `${{el.tagName.toLowerCase()}}:nth-of-type(${{index}})`;
            }}
        }}

        interactive.push(item);
        count++;
    }});

    const headings = Array.from(root.querySelectorAll('h1, h2, h3'))
        .map(h => h.textContent.trim())
        .filter(t => t.length > 0)
        .slice(0, 10);

    // Text summary — extract visible text from the scoped root
    const scopeContent = root.querySelector('main, [role="main"], article, .readme, #readme') || root;
    const paragraphs = Array.from(scopeContent.querySelectorAll('p, li, dd, blockquote'))
        .filter(el => {{
            if (!el.offsetParent && el.style.position !== 'fixed') return false;
            const text = el.textContent.trim();
            return text.length > 20;
        }})
        .map(el => el.textContent.trim())
        .slice(0, 5);
    const textSummary = paragraphs.join(' ').slice(0, 600);

    return {{
        url: window.location.href,
        title: document.title,
        interactive: interactive,
        headings: headings,
        text_summary: textSummary || document.title,
    }};
}})()"#,
                    sel_json = sel_json,
                )
            }
            None => EXTRACT_PAGE_MAP_JS.to_string(),
        };
        let result = self.page.evaluate(js).await?;
        let map: PageMap = result
            .into_value()
            .map_err(|e| RayoError::Cdp(format!("Failed to deserialize page map: {e:?}")))?;
        *self.page_map_cache.lock().await = Some(map.clone());
        Ok(map)
    }

    /// Get text content of the page or specific elements.
    ///
    /// When a selector is provided, uses `querySelectorAll` and joins all matches
    /// with newlines. Results are capped at `max_elements`; if exceeded, a
    /// `[truncated: N more elements matched]` notice is appended.
    pub async fn text_content(
        &self,
        selector: Option<&str>,
        max_elements: usize,
    ) -> Result<String, RayoError> {
        let _span = self
            .profiler
            .start_span("text_content", SpanCategory::DomRead);
        let js = match selector {
            Some(sel) => format!(
                r#"(() => {{
                    const els = document.querySelectorAll({sel_json});
                    if (els.length === 0) return "";
                    const max = {max};
                    const texts = [];
                    for (let i = 0; i < Math.min(els.length, max); i++) {{
                        const t = (els[i].textContent || "").trim();
                        if (t) texts.push(t);
                    }}
                    let result = texts.join("\n");
                    if (els.length > max) {{
                        result += "\n[truncated: " + (els.length - max) + " more elements matched]";
                    }}
                    return result;
                }})()"#,
                sel_json = serde_json::to_string(sel).unwrap(),
                max = max_elements,
            ),
            None => "document.body.innerText".to_string(),
        };
        let result = self.page.evaluate(js).await?;
        Ok(result.into_value::<String>().unwrap_or_default())
    }

    /// Take a screenshot, returns base64-encoded JPEG.
    pub async fn screenshot(&self, full_page: bool) -> Result<String, RayoError> {
        let _span = self
            .profiler
            .start_span("screenshot", SpanCategory::Screenshot);
        let clip = if full_page {
            None
        } else {
            let dims = self
                .page
                .evaluate("[window.innerWidth, window.innerHeight]")
                .await?;
            let arr: Vec<f64> = dims.into_value().unwrap_or_default();
            let (w, h) = (
                arr.first().copied().unwrap_or(1280.0),
                arr.get(1).copied().unwrap_or(720.0),
            );
            Some(Viewport {
                x: 0.0,
                y: 0.0,
                width: w,
                height: h,
                scale: 1.0,
            })
        };
        let params = CaptureScreenshotParams {
            format: Some(CaptureScreenshotFormat::Jpeg),
            quality: Some(80),
            clip,
            optimize_for_speed: Some(true),
            ..Default::default()
        };
        let bytes = self.page.screenshot(params).await?;
        use base64::Engine;
        Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
    }

    /// Screenshot MIME type for MCP responses.
    pub fn screenshot_mime() -> &'static str {
        "image/jpeg"
    }

    /// Click an element by selector or page map ID.
    /// Uses CDP Input.dispatchMouseEvent via chromiumoxide for real mouse events.
    pub async fn click(&self, selector: Option<&str>, id: Option<usize>) -> Result<(), RayoError> {
        self.click_raw(selector, id).await?;
        self.invalidate_after_mutation().await;
        Ok(())
    }

    /// Internal click without cache invalidation — used by batch executor
    /// to defer all invalidation to a single pass at the end.
    async fn click_raw(&self, selector: Option<&str>, id: Option<usize>) -> Result<(), RayoError> {
        let sel = self.resolve_selector(selector, id).await?;
        let _span = self.profiler.start_span(
            format!("click({})", truncate(&sel, 40)),
            SpanCategory::DomMutate,
        );
        // Use CDP Input events via chromiumoxide Element API
        // Element::click() internally calls scroll_into_view() + dispatchMouseEvent
        let element =
            self.page
                .find_element(&sel)
                .await
                .map_err(|e| RayoError::ElementNotFound {
                    selector: format!("{sel}: {e}"),
                })?;
        element
            .click()
            .await
            .map_err(|e| RayoError::Cdp(format!("click failed: {e}")))?;
        Ok(())
    }

    /// Type text into an element.
    /// Uses CDP Input.dispatchKeyEvent via chromiumoxide for real keystroke events.
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
        let element =
            self.page
                .find_element(&sel)
                .await
                .map_err(|e| RayoError::ElementNotFound {
                    selector: format!("{sel}: {e}"),
                })?;
        // Click to focus the element
        element
            .click()
            .await
            .map_err(|e| RayoError::Cdp(format!("focus click failed: {e}")))?;
        if clear {
            // Clear existing content using non-deprecated API
            let clear_js = format!(
                r#"(() => {{
                    const el = document.querySelector({sel_json});
                    if (el) {{
                        el.value = '';
                        el.dispatchEvent(new Event('input', {{bubbles: true}}));
                        el.dispatchEvent(new Event('change', {{bubbles: true}}));
                    }}
                }})()"#,
                sel_json = serde_json::to_string(&sel).unwrap()
            );
            if let Err(e) = self.page.evaluate(clear_js).await {
                tracing::warn!("Failed to clear input: {e}");
            }
        }
        // Type each character via CDP Input.dispatchKeyEvent
        element
            .type_str(text)
            .await
            .map_err(|e| RayoError::Cdp(format!("type failed: {e}")))?;
        self.invalidate_after_mutation().await;
        Ok(())
    }

    /// Press a key on an element or the document.
    /// Uses CDP Input.dispatchKeyEvent via chromiumoxide for real key events.
    /// Key names follow CDP conventions: "Enter", "Tab", "Escape", "ArrowDown", etc.
    pub async fn press_key(
        &self,
        selector: Option<&str>,
        id: Option<usize>,
        key: &str,
    ) -> Result<(), RayoError> {
        let _span = self.profiler.start_span(
            format!("press_key({})", truncate(key, 20)),
            SpanCategory::DomMutate,
        );
        if selector.is_some() || id.is_some() {
            let sel = self.resolve_selector(selector, id).await?;
            let element =
                self.page
                    .find_element(&sel)
                    .await
                    .map_err(|e| RayoError::ElementNotFound {
                        selector: format!("{sel}: {e}"),
                    })?;
            element
                .press_key(key)
                .await
                .map_err(|e| RayoError::Cdp(format!("press_key failed: {e}")))?;
        } else {
            // No selector/id — dispatch key press on the document body
            let element =
                self.page.find_element("body").await.map_err(|e| {
                    RayoError::Cdp(format!("could not find body for key press: {e}"))
                })?;
            element
                .press_key(key)
                .await
                .map_err(|e| RayoError::Cdp(format!("press_key failed: {e}")))?;
        }
        self.invalidate_after_mutation().await;
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
        self.invalidate_after_mutation().await;
        Ok(())
    }

    /// Execute a batch of actions.
    /// When `abort_on_failure` is true, remaining actions are skipped after the first failure.
    pub async fn execute_batch(
        &self,
        actions: Vec<BatchAction>,
        abort_on_failure: bool,
    ) -> Result<BatchResult, RayoError> {
        let _span = self
            .profiler
            .start_span(format!("batch({})", actions.len()), SpanCategory::Batch);
        let start = std::time::Instant::now();
        let mut results = Vec::with_capacity(actions.len());
        let mut succeeded = 0usize;
        let mut failed = 0usize;

        for (i, action) in actions.iter().enumerate() {
            let action_start = std::time::Instant::now();
            let res = match action {
                BatchAction::Click { target } => {
                    let (sel, id) = target_to_selector_id(target);
                    self.click_raw(sel, id).await.map(|_| None)
                }
                BatchAction::Type { target, value } => {
                    let (sel, id) = target_to_selector_id(target);
                    self.type_text(sel, id, value, true).await.map(|_| None)
                }
                BatchAction::Select { target, value } => {
                    let (sel, id) = target_to_selector_id(target);
                    self.select_option(sel, id, value).await.map(|_| None)
                }
                BatchAction::Goto { url } => self.goto(url).await.map(|_| None),
                BatchAction::Screenshot { full_page } => self
                    .screenshot(*full_page)
                    .await
                    .map(|b64| Some(serde_json::Value::String(b64))),
                BatchAction::WaitFor { target, timeout_ms } => {
                    let (sel, id) = target_to_selector_id(target);
                    let selector = self.resolve_selector(sel, id).await?;
                    self.wait_for_selector(&selector, *timeout_ms)
                        .await
                        .map(|_| None)
                }
                BatchAction::Press { target, key } => {
                    let (sel, id) = if let Some(t) = target {
                        target_to_selector_id(t)
                    } else {
                        (None, None)
                    };
                    self.press_key(sel, id, key).await.map(|_| None)
                }
                BatchAction::Scroll { target, x, y } => {
                    if let Some(t) = target {
                        let (sel, id) = target_to_selector_id(t);
                        let selector = self.resolve_selector(sel, id).await?;
                        let js = format!(
                            "document.querySelector({}).scrollIntoView({{block:'center'}})",
                            serde_json::to_string(&selector).unwrap(),
                        );
                        self.page
                            .evaluate(js)
                            .await
                            .map(|_| None)
                            .map_err(RayoError::from)
                    } else {
                        let js = format!("window.scrollTo({x},{y})");
                        self.page
                            .evaluate(js)
                            .await
                            .map(|_| None)
                            .map_err(RayoError::from)
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
                    if abort_on_failure {
                        // Mark remaining actions as skipped
                        for (j, remaining) in actions.iter().enumerate().skip(i + 1) {
                            results.push(BatchActionResult {
                                index: j,
                                action: action_name(remaining).to_string(),
                                success: false,
                                error: Some("Skipped (abort_on_failure)".to_string()),
                                data: None,
                                duration_ms: 0.0,
                            });
                            failed += 1;
                        }
                        break;
                    }
                }
            }
        }

        // Single cache invalidation after all batch actions complete
        self.invalidate_after_mutation().await;

        Ok(BatchResult {
            results,
            total_duration_ms: start.elapsed().as_secs_f64() * 1000.0,
            succeeded,
            failed,
        })
    }

    /// Wait for a selector to appear using a MutationObserver-based approach.
    /// Instead of polling, this injects a Promise that resolves immediately if
    /// the element exists, or sets up a MutationObserver to detect when it appears.
    pub async fn wait_for_selector(
        &self,
        selector: &str,
        timeout_ms: u64,
    ) -> Result<(), RayoError> {
        let _span = self.profiler.start_span(
            format!("wait({})", truncate(selector, 40)),
            SpanCategory::Wait,
        );
        let sel_json = serde_json::to_string(selector).unwrap();
        let js = format!(
            r#"new Promise((resolve, reject) => {{
                const sel = {sel_json};
                const el = document.querySelector(sel);
                if (el) {{ resolve(true); return; }}
                const observer = new MutationObserver(() => {{
                    if (document.querySelector(sel)) {{
                        observer.disconnect();
                        resolve(true);
                    }}
                }});
                observer.observe(document.body || document.documentElement, {{ childList: true, subtree: true }});
                setTimeout(() => {{
                    observer.disconnect();
                    reject(new Error('timeout'));
                }}, {timeout_ms});
            }})"#
        );

        self.page.evaluate(js).await.map_err(|e| {
            let err_str = e.to_string();
            if err_str.contains("timeout") {
                RayoError::Timeout {
                    what: selector.to_string(),
                    ms: timeout_ms,
                }
            } else {
                RayoError::from(e)
            }
        })?;
        Ok(())
    }

    /// Evaluate JavaScript on the page.
    pub async fn evaluate(&self, js: &str) -> Result<serde_json::Value, RayoError> {
        let _span = self
            .profiler
            .start_span("evaluate", SpanCategory::CdpCommand);
        let result = self.page.evaluate(js).await?;
        Ok(result.into_value().unwrap_or(serde_json::Value::Null))
    }

    /// Set cookies on the page.
    pub async fn set_cookies(&self, cookies: Vec<SetCookie>) -> Result<(), RayoError> {
        let _span = self.profiler.start_span(
            format!("set_cookies({})", cookies.len()),
            SpanCategory::CdpCommand,
        );
        let cdp_cookies: Vec<CookieParam> = cookies.into_iter().map(to_cdp_cookie).collect();
        self.page
            .set_cookies(cdp_cookies)
            .await
            .map_err(|e| RayoError::CookieError(format!("Failed to set cookies: {e}")))?;
        Ok(())
    }

    /// Get all cookies for the current page.
    pub async fn get_cookies(&self) -> Result<Vec<CookieInfo>, RayoError> {
        let _span = self
            .profiler
            .start_span("get_cookies", SpanCategory::CdpCommand);
        let cookies = self
            .page
            .get_cookies()
            .await
            .map_err(|e| RayoError::CookieError(format!("Failed to get cookies: {e}")))?;
        Ok(cookies
            .into_iter()
            .map(|c| CookieInfo {
                name: c.name,
                value: c.value,
                domain: c.domain,
                path: c.path,
                secure: c.secure,
                http_only: c.http_only,
                same_site: c.same_site.map(|s| format!("{s:?}")),
                expires: c.expires,
            })
            .collect())
    }

    /// Delete a specific cookie by name, optionally scoped to a domain.
    pub async fn delete_cookie(&self, name: &str, domain: Option<&str>) -> Result<(), RayoError> {
        let _span = self
            .profiler
            .start_span(format!("delete_cookie({})", name), SpanCategory::CdpCommand);
        let mut params = DeleteCookiesParams::new(name);
        if let Some(d) = domain {
            params.domain = Some(d.to_string());
        }
        self.page
            .execute(params)
            .await
            .map_err(|e| RayoError::CookieError(format!("Failed to delete cookie: {e}")))?;
        Ok(())
    }

    /// Clear all cookies.
    pub async fn clear_cookies(&self) -> Result<(), RayoError> {
        let _span = self
            .profiler
            .start_span("clear_cookies", SpanCategory::CdpCommand);
        self.page
            .execute(ClearBrowserCookiesParams {})
            .await
            .map_err(|e| RayoError::CookieError(format!("Failed to clear cookies: {e}")))?;
        Ok(())
    }

    /// Enable passive network monitoring via the CDP Network domain.
    ///
    /// Uses `Network.enable` to passively observe traffic without intercepting it.
    /// Listens for `Network.requestWillBeSent` and `Network.responseReceived` events
    /// to record requests and their response statuses. Requests flow normally with
    /// zero added latency — this is the right mode for capture-only use cases.
    pub async fn enable_network_monitoring(
        &self,
        network: Arc<Mutex<NetworkInterceptor>>,
    ) -> Result<(), RayoError> {
        // Enable the Network domain for passive monitoring
        self.page
            .execute(NetworkEnableParams::default())
            .await
            .map_err(|e| RayoError::Cdp(format!("Network.enable failed: {e}")))?;

        // Subscribe to requestWillBeSent events
        let mut request_events = self
            .page
            .event_listener::<EventRequestWillBeSent>()
            .await
            .map_err(|e| {
                RayoError::Cdp(format!(
                    "Failed to listen for Network.requestWillBeSent: {e}"
                ))
            })?;

        // Subscribe to responseReceived events
        let mut response_events = self
            .page
            .event_listener::<EventResponseReceived>()
            .await
            .map_err(|e| {
                RayoError::Cdp(format!(
                    "Failed to listen for Network.responseReceived: {e}"
                ))
            })?;

        // Spawn task for requestWillBeSent — records new requests
        let network_for_requests = Arc::clone(&network);
        tokio::spawn(async move {
            while let Some(event) = request_events.next().await {
                let url = event.request.url.clone();
                let method = event.request.method.clone();
                let resource_type_str = event
                    .r#type
                    .as_ref()
                    .map(|t| t.as_ref().to_string())
                    .unwrap_or_else(|| "Other".to_string());
                let request_id = event.request_id.inner().to_string();

                // Extract request headers
                let headers: Vec<(String, String)> = event
                    .request
                    .headers
                    .inner()
                    .as_object()
                    .map(|obj| {
                        obj.iter()
                            .filter_map(|(k, v)| {
                                v.as_str().map(|val| (k.clone(), val.to_string()))
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let mut net = network_for_requests.lock().await;
                if net.is_capturing() {
                    net.record_request(CapturedRequest {
                        url,
                        method,
                        resource_type: resource_type_str,
                        status: None, // filled in by responseReceived
                        headers,
                        timestamp_ms: timestamp_now_ms(),
                        request_id: Some(request_id),
                    });
                }
            }
        });

        // Spawn task for responseReceived — updates status on existing requests
        let network_for_responses = Arc::clone(&network);
        tokio::spawn(async move {
            while let Some(event) = response_events.next().await {
                let request_id = event.request_id.inner().to_string();
                let status = event.response.status;

                let mut net = network_for_responses.lock().await;
                if net.is_capturing() {
                    net.update_request_status(&request_id, status);
                }
            }
        });

        Ok(())
    }

    /// Enable CDP Fetch-domain interception and wire events to the NetworkInterceptor.
    ///
    /// Subscribes to `Fetch.requestPaused` events. For each paused request the handler
    /// checks block rules, mock rules, and capture state in the shared `NetworkInterceptor`,
    /// then responds with `failRequest`, `fulfillRequest`, or `continueRequest` accordingly.
    ///
    /// This is only needed when block or mock rules are active. For capture-only,
    /// use `enable_network_monitoring()` instead.
    pub async fn enable_network_interception(
        &self,
        network: Arc<Mutex<NetworkInterceptor>>,
    ) -> Result<(), RayoError> {
        // Enable the Fetch domain — intercept all requests
        self.page
            .execute(FetchEnableParams {
                patterns: None, // intercept everything
                handle_auth_requests: None,
            })
            .await
            .map_err(|e| RayoError::Cdp(format!("Fetch.enable failed: {e}")))?;

        // Subscribe to requestPaused events
        let mut events = self
            .page
            .event_listener::<EventRequestPaused>()
            .await
            .map_err(|e| RayoError::Cdp(format!("Failed to listen for Fetch events: {e}")))?;

        // Clone the inner CdpPage handle for the spawned task
        let page = self.page.clone();

        tokio::spawn(async move {
            while let Some(event) = events.next().await {
                let event = Arc::new(event);
                let request_id = event.request_id.clone();
                let url = event.request.url.clone();
                let method = event.request.method.clone();
                let resource_type_str = event.resource_type.as_ref().to_string();

                // Extract request headers as Vec<(String, String)>
                let headers: Vec<(String, String)> = event
                    .request
                    .headers
                    .inner()
                    .as_object()
                    .map(|obj| {
                        obj.iter()
                            .filter_map(|(k, v)| {
                                v.as_str().map(|val| (k.clone(), val.to_string()))
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let mut net = network.lock().await;

                // 1. Check block rules
                if net.should_block(&url, Some(&resource_type_str)) {
                    drop(net);
                    if let Err(e) = page
                        .execute(FailRequestParams::new(
                            request_id,
                            ErrorReason::BlockedByClient,
                        ))
                        .await
                    {
                        tracing::warn!("Fetch.failRequest failed: {e}");
                    }
                    continue;
                }

                // 2. Check mock rules
                if let Some(mock) = net.find_mock(&url, Some(&resource_type_str)).cloned() {
                    // Record the request if capturing
                    if net.is_capturing() {
                        net.record_request(CapturedRequest {
                            url: url.clone(),
                            method,
                            resource_type: resource_type_str,
                            status: Some(mock.status as i64),
                            headers,
                            timestamp_ms: timestamp_now_ms(),
                            request_id: None,
                        });
                    }
                    drop(net);

                    let response_headers: Vec<FetchHeaderEntry> = mock
                        .headers
                        .iter()
                        .map(|(k, v)| FetchHeaderEntry::new(k.clone(), v.clone()))
                        .collect();

                    use base64::Engine;
                    let body_b64 =
                        base64::engine::general_purpose::STANDARD.encode(mock.body.as_bytes());

                    let mut params =
                        FulfillRequestParams::new(request_id, mock.status as i64);
                    params.response_headers = Some(response_headers);
                    params.body = Some(body_b64.into());

                    if let Err(e) = page.execute(params).await {
                        tracing::warn!("Fetch.fulfillRequest failed: {e}");
                    }
                    continue;
                }

                // 3. Record and continue
                if net.is_capturing() {
                    net.record_request(CapturedRequest {
                        url,
                        method,
                        resource_type: resource_type_str,
                        status: None, // status unknown at request stage
                        headers,
                        timestamp_ms: timestamp_now_ms(),
                        request_id: None,
                    });
                }
                drop(net);

                if let Err(e) = page
                    .execute(ContinueRequestParams::new(request_id))
                    .await
                {
                    tracing::warn!("Fetch.continueRequest failed: {e}");
                }
            }
        });

        Ok(())
    }

    /// Resolve a selector from either a CSS selector or a page map element ID.
    async fn resolve_selector(
        &self,
        selector: Option<&str>,
        id: Option<usize>,
    ) -> Result<String, RayoError> {
        if let Some(sel) = selector {
            // Check selector cache for a validated version
            let mut sc = self.selector_cache.lock().await;
            if let Some(cached) = sc.get(sel) {
                return Ok(cached.selector.clone());
            }
            return Ok(sel.to_string());
        }
        if let Some(element_id) = id {
            let cache_key = format!("id:{element_id}");

            // Check selector cache first
            {
                let mut sc = self.selector_cache.lock().await;
                if let Some(cached) = sc.get(&cache_key) {
                    return Ok(cached.selector.clone());
                }
            }

            // Look up from cached page map
            let cache = self.page_map_cache.lock().await;
            if let Some(map) = cache.as_ref()
                && let Some(el) = map.interactive.iter().find(|e| e.id == element_id)
            {
                let resolved = el.selector.clone();
                drop(cache);
                self.selector_cache
                    .lock()
                    .await
                    .put(cache_key, resolved.clone());
                return Ok(resolved);
            }
            drop(cache);
            // Refresh page map and retry
            let map = self.page_map(None).await?;
            if let Some(el) = map.interactive.iter().find(|e| e.id == element_id) {
                let resolved = el.selector.clone();
                self.selector_cache
                    .lock()
                    .await
                    .put(cache_key, resolved.clone());
                return Ok(resolved);
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
        BatchAction::Press { .. } => "press",
        BatchAction::Goto { .. } => "goto",
        BatchAction::Screenshot { .. } => "screenshot",
        BatchAction::WaitFor { .. } => "wait_for",
        BatchAction::Scroll { .. } => "scroll",
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    // Find the largest char boundary <= max to avoid panicking on multi-byte UTF-8
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Current time in milliseconds since UNIX epoch, for captured request timestamps.
fn timestamp_now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
        * 1000.0
}

/// Convert rayo-owned SetCookie to chromiumoxide CookieParam.
fn to_cdp_cookie(c: SetCookie) -> CookieParam {
    let mut cp = CookieParam::new(c.name, c.value);
    cp.domain = c.domain;
    cp.path = c.path;
    cp.url = c.url;
    cp.secure = c.secure;
    cp.http_only = c.http_only;
    cp.same_site = c.same_site.map(|s| match s {
        SameSite::Strict => CookieSameSite::Strict,
        SameSite::Lax => CookieSameSite::Lax,
        SameSite::None => CookieSameSite::None,
    });
    cp.expires = c.expires.map(TimeSinceEpoch::new);
    cp
}
