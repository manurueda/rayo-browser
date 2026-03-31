//! Browser and Page wrappers around chromiumoxide.
//!
//! Adds rayo's AI-native features on top: page maps, batch execution,
//! selector caching, profiling.

use std::sync::Arc;

use crate::cookie::{CookieInfo, CookieSetResult, SameSite, SetCookie};
use crate::network::{CapturedRequest, NetworkInterceptor};
use chromiumoxide::browser::{Browser as CdpBrowser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::css::{
    EnableParams as CssEnableParams, GetComputedStyleForNodeParams, GetMatchedStylesForNodeParams,
};
use chromiumoxide::cdp::browser_protocol::dom::{
    GetBoxModelParams, GetDocumentParams, QuerySelectorParams,
};
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
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;

use crate::inspect::{
    self, AccessibilityInfo, AppliedRule, DiffCache, InspectOptions, InspectResult, InspectTarget,
    StyleChange, StyleDiff, VisibilityInfo,
};

use crate::batch::{ActionTarget, BatchAction, BatchActionResult, BatchResult};
use crate::error::RayoError;
use crate::page_map::{EXTRACT_PAGE_MAP_JS, PageMap};
use crate::selector_cache::SelectorCache;
use rayo_profiler::{Profiler, SpanCategory};

/// Viewport configuration for the browser window.
#[derive(Debug, Clone)]
pub struct ViewportConfig {
    pub width: u32,
    pub height: u32,
}

impl Default for ViewportConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
        }
    }
}

/// Result of a navigation with auto-auth.
#[derive(Debug, Clone)]
pub struct NavigationResult {
    /// The page map after navigation completed.
    pub map: crate::page_map::PageMap,
    /// The URL that was originally requested.
    pub requested_url: String,
    /// True if the final URL differs meaningfully from the requested URL.
    pub redirected: bool,
    /// Whether auto-auth was attempted.
    pub auto_auth: AutoAuthStatus,
}

/// What happened with auto-auth during navigation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoAuthStatus {
    /// No auth wall was detected.
    NotNeeded,
    /// Auth wall detected, cookies imported, retry succeeded.
    Succeeded,
    /// Auth wall detected, but cookie import or retry failed.
    Failed,
}

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

    /// Launch with a shared profiler and custom viewport.
    pub async fn launch_with_config(
        profiler: Profiler,
        viewport: ViewportConfig,
    ) -> Result<Self, RayoError> {
        let mut browser = Self::launch_viewport(viewport).await?;
        browser.profiler = profiler;
        Ok(browser)
    }

    /// Launch a new headless Chrome instance with default viewport (1280x720).
    pub async fn launch() -> Result<Self, RayoError> {
        Self::launch_viewport(ViewportConfig::default()).await
    }

    /// Launch a new headless Chrome instance with custom viewport.
    pub async fn launch_viewport(viewport: ViewportConfig) -> Result<Self, RayoError> {
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
            .window_size(viewport.width, viewport.height)
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
            css_enabled: AtomicBool::new(false),
            diff_cache: Arc::new(Mutex::new(DiffCache::new(256))),
        })
    }

    /// Get the profiler.
    pub fn profiler(&self) -> &Profiler {
        &self.profiler
    }

    /// Gracefully close the browser and clean up.
    ///
    /// Sends a CDP close command to Chrome, waits up to 5 seconds for the
    /// process to exit, then force-kills if needed. Also waits for the
    /// handler task to finish so no background work is left running.
    ///
    /// Prefer this over just dropping `RayoBrowser` when you have an async
    /// context (e.g. MCP server shutdown).
    pub async fn close(mut self) {
        // 1. Ask Chrome to close gracefully via CDP
        if let Err(e) = self.browser.close().await {
            tracing::warn!("CDP close failed: {e}, will force-kill");
        }

        // 2. Wait for Chrome process to exit (up to 5s)
        let wait_result =
            tokio::time::timeout(std::time::Duration::from_secs(5), self.browser.wait()).await;

        match wait_result {
            Ok(Ok(_)) => tracing::debug!("Chrome exited cleanly"),
            Ok(Err(e)) => {
                tracing::warn!("Chrome wait error: {e}, force-killing");
                let _ = self.browser.kill().await;
            }
            Err(_) => {
                tracing::warn!("Chrome did not exit within 5s, force-killing");
                let _ = self.browser.kill().await;
            }
        }

        // 3. Stop the handler task
        self.handler_task.abort();
        let _ = (&mut self.handler_task).await;
    }
}

impl Drop for RayoBrowser {
    fn drop(&mut self) {
        // Safety net: abort the handler task if close() was not called.
        // This is the best we can do without an async context.
        // chromiumoxide's own Drop will kill_on_drop the Chrome process.
        self.handler_task.abort();
    }
}

/// Rayo page wrapper with AI-native features.
pub struct RayoPage {
    page: CdpPage,
    selector_cache: Arc<Mutex<SelectorCache>>,
    profiler: Profiler,
    page_map_cache: Arc<Mutex<Option<PageMap>>>,
    /// Whether CSS domain has been enabled on this page (once per session).
    css_enabled: AtomicBool,
    /// Separate cache for style diff — survives DOM mutations.
    diff_cache: Arc<Mutex<DiffCache>>,
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
        self.goto_raw(url).await?;
        self.invalidate_after_mutation().await;
        Ok(())
    }

    /// Navigate to a URL with transparent auth.
    ///
    /// 1. Loads persisted cookies for the target domain from `~/.rayo/cookies/`
    /// 2. Navigates to the URL
    /// 3. Detects auth walls (login redirects or password forms)
    /// 4. If an auth wall is detected, auto-imports cookies from the user's
    ///    default browser, persists them, and retries navigation
    ///
    /// All errors in the auto-auth path are warnings, not failures.
    /// If auto-auth fails, the agent still sees the page (just the login page).
    #[cfg(feature = "cookie-import")]
    pub async fn goto_with_auto_auth(
        &self,
        url: &str,
        llm_checker: Option<&crate::auth::LlmAuthChecker>,
    ) -> Result<NavigationResult, RayoError> {
        let _span = self.profiler.start_span(
            format!("goto_with_auto_auth({})", truncate(url, 60)),
            SpanCategory::Auth,
        );

        let domain = crate::auth::extract_domain(url).unwrap_or_default();

        // Step 1: Load persisted cookies for this domain
        if !domain.is_empty()
            && let Some(cookies) = crate::persist::load_domain_cookies(&domain)
        {
            let set_cookies: Vec<SetCookie> = cookies
                .into_iter()
                .map(|c| SetCookie {
                    name: c.name,
                    value: c.value,
                    domain: Some(c.domain),
                    path: Some(c.path),
                    url: None,
                    secure: Some(c.secure),
                    http_only: Some(c.http_only),
                    same_site: c.same_site.as_deref().and_then(SameSite::parse),
                    expires: if c.expires > 0.0 {
                        Some(c.expires)
                    } else {
                        None
                    },
                })
                .collect();
            if !set_cookies.is_empty() {
                tracing::debug!(
                    "Injecting {} persisted cookies for {domain}",
                    set_cookies.len()
                );
                if let Err(e) = self.set_cookies(set_cookies).await {
                    tracing::warn!("Failed to inject persisted cookies: {e}");
                }
            }
        }

        // Step 2: Navigate
        self.goto(url).await?;

        // Step 3: Check for auth wall using confidence scoring
        let final_url = self.url().await.unwrap_or_default();
        let map = self.page_map(None).await?;
        let redirected = crate::auth::is_meaningful_redirect(url, &map.url);

        let auth = crate::auth::detect_auth_wall(url, &final_url, &map);

        // Determine if auth wall is present using tiered approach
        let is_auth = match auth.confidence {
            c if c >= 0.5 => true,
            c if c < 0.2 => false,
            _ => {
                // Uncertain zone — try LLM if available
                if let Some(checker) = llm_checker {
                    if let Ok(screenshot) = self.screenshot(false).await {
                        let result = tokio::time::timeout(
                            std::time::Duration::from_secs(5),
                            checker(screenshot),
                        )
                        .await;
                        match result {
                            Ok(Some(is_login)) => is_login,
                            _ => auth.confidence >= 0.35,
                        }
                    } else {
                        auth.confidence >= 0.35
                    }
                } else {
                    auth.confidence >= 0.35
                }
            }
        };

        if is_auth {
            tracing::info!(
                "Auth wall detected at {final_url} (confidence: {:.0}%, signals: {:?})",
                auth.confidence * 100.0,
                auth.signals,
            );

            // Step 4: Auto-detect browser and import cookies
            if let Some(browser) =
                crate::detect::default_browser().or_else(crate::detect::find_available_browser)
            {
                match crate::cookie_import::import_cookies(browser, Some(&domain), None) {
                    Ok(import) if !import.cookies.is_empty() => {
                        tracing::info!(
                            "Imported {} cookies from {browser} for {domain} (profile: {}, {}/{} from DB)",
                            import.cookies.len(),
                            import.profile_used,
                            import.cookies.len(),
                            import.found_in_db,
                        );

                        // Inject imported cookies
                        if let Err(e) = self.set_cookies(import.cookies).await {
                            tracing::warn!("Failed to inject imported cookies: {e}");
                            return Ok(NavigationResult {
                                map,
                                requested_url: url.to_string(),
                                redirected,
                                auto_auth: AutoAuthStatus::Failed,
                            });
                        }

                        // Retry navigation
                        if let Err(e) = self.goto(url).await {
                            tracing::warn!("Retry navigation failed: {e}");
                            return Ok(NavigationResult {
                                map,
                                requested_url: url.to_string(),
                                redirected,
                                auto_auth: AutoAuthStatus::Failed,
                            });
                        }

                        // Persist cookies for next session
                        if let Ok(cookie_infos) = self.get_cookies().await {
                            let domain_cookies: Vec<_> = cookie_infos
                                .into_iter()
                                .filter(|c| crate::cookie::matches_domain(&c.domain, &domain))
                                .collect();
                            if let Err(e) =
                                crate::persist::save_domain_cookies(&domain, &domain_cookies)
                            {
                                tracing::warn!("Failed to persist cookies: {e}");
                            }
                        }

                        let retry_map = self.page_map(None).await?;
                        let retry_redirected =
                            crate::auth::is_meaningful_redirect(url, &retry_map.url);
                        let auto_auth = if retry_redirected {
                            AutoAuthStatus::Failed
                        } else {
                            AutoAuthStatus::Succeeded
                        };
                        return Ok(NavigationResult {
                            map: retry_map,
                            requested_url: url.to_string(),
                            redirected: retry_redirected,
                            auto_auth,
                        });
                    }
                    Ok(import) => {
                        tracing::debug!(
                            "No cookies found in {browser} for {domain} (profile: {}, available: {:?})",
                            import.profile_used,
                            import.available_profiles,
                        );
                    }
                    Err(e) => {
                        tracing::warn!("Auto cookie import from {browser} failed: {e}");
                    }
                }
            } else {
                tracing::debug!("No browser detected for cookie import");
            }

            return Ok(NavigationResult {
                map,
                requested_url: url.to_string(),
                redirected,
                auto_auth: AutoAuthStatus::Failed,
            });
        }

        Ok(NavigationResult {
            map,
            requested_url: url.to_string(),
            redirected,
            auto_auth: AutoAuthStatus::NotNeeded,
        })
    }

    /// Navigate with auto-auth (no-op when cookie-import feature is disabled).
    #[cfg(not(feature = "cookie-import"))]
    pub async fn goto_with_auto_auth(
        &self,
        url: &str,
        _llm_checker: Option<&crate::auth::LlmAuthChecker>,
    ) -> Result<NavigationResult, RayoError> {
        self.goto(url).await?;
        let map = self.page_map(None).await?;
        let redirected = crate::auth::is_meaningful_redirect(url, &map.url);
        Ok(NavigationResult {
            map,
            requested_url: url.to_string(),
            redirected,
            auto_auth: AutoAuthStatus::NotNeeded,
        })
    }

    /// Internal goto without cache invalidation — used by batch executor
    /// to defer all invalidation to a single pass at the end.
    async fn goto_raw(&self, url: &str) -> Result<(), RayoError> {
        let _span = self.profiler.start_span(
            format!("goto({})", truncate(url, 60)),
            SpanCategory::Navigation,
        );
        self.page.goto(url).await?;
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
        const role = el.getAttribute('role');
        if (text && text.length < 100 && (
            el.tagName === 'BUTTON' ||
            el.tagName === 'A' ||
            role === 'button' ||
            role === 'link' ||
            role === 'tab'
        )) {{
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

        // Element state
        const state = [];
        if (el.disabled) state.push('disabled');
        if (el.readOnly) state.push('readonly');
        if (el.required) state.push('required');
        if (el.checked) state.push('checked');
        if (el.hidden || (el.type === 'hidden')) state.push('hidden');
        if (state.length > 0) item.state = state;

        interactive.push(item);
        count++;
    }});

    const totalInteractive = elements.length;

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
        total_interactive: totalInteractive > MAX_ELEMENTS ? totalInteractive : undefined,
        truncated: totalInteractive > MAX_ELEMENTS ? true : undefined,
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

    /// Take a PNG screenshot (lossless, for visual testing).
    /// Returns raw PNG bytes (not base64).
    pub async fn screenshot_png(&self, full_page: bool) -> Result<Vec<u8>, RayoError> {
        let _span = self
            .profiler
            .start_span("screenshot_png", SpanCategory::Screenshot);
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
            format: Some(CaptureScreenshotFormat::Png),
            quality: None,
            clip,
            optimize_for_speed: Some(false),
            ..Default::default()
        };
        Ok(self.page.screenshot(params).await?)
    }

    /// Take a screenshot of a specific element by CSS selector.
    /// Uses the element's bounding box as the clip region.
    /// Returns raw PNG bytes.
    pub async fn screenshot_element(&self, selector: &str) -> Result<Vec<u8>, RayoError> {
        let _span = self
            .profiler
            .start_span("screenshot_element", SpanCategory::Screenshot);

        // Get bounding box via JavaScript
        let js = format!(
            r#"(() => {{
                const el = document.querySelector({sel});
                if (!el) return null;
                el.scrollIntoView({{ block: 'center', behavior: 'instant' }});
                const rect = el.getBoundingClientRect();
                return {{ x: rect.x, y: rect.y, width: rect.width, height: rect.height }};
            }})()"#,
            sel = serde_json::to_string(selector).unwrap_or_default()
        );
        let result = self.page.evaluate(js).await?;
        let bbox: serde_json::Value = result.into_value().unwrap_or_default();

        if bbox.is_null() {
            return Err(RayoError::ElementNotFound {
                selector: selector.to_string(),
            });
        }

        let x = bbox["x"].as_f64().unwrap_or(0.0);
        let y = bbox["y"].as_f64().unwrap_or(0.0);
        let w = bbox["width"].as_f64().unwrap_or(0.0);
        let h = bbox["height"].as_f64().unwrap_or(0.0);

        if w <= 0.0 || h <= 0.0 {
            return Err(RayoError::ElementNotFound {
                selector: format!("{selector} (zero dimensions: {w}x{h})"),
            });
        }

        let params = CaptureScreenshotParams {
            format: Some(CaptureScreenshotFormat::Png),
            quality: None,
            clip: Some(Viewport {
                x,
                y,
                width: w,
                height: h,
                scale: 1.0,
            }),
            optimize_for_speed: Some(false),
            ..Default::default()
        };
        Ok(self.page.screenshot(params).await?)
    }

    /// Freeze CSS animations and transitions for stable visual testing screenshots.
    /// Returns a guard that removes the injected CSS when dropped.
    pub async fn freeze_animations(&self) -> Result<(), RayoError> {
        let _span = self
            .profiler
            .start_span("freeze_animations", SpanCategory::DomMutate);
        self.page
            .evaluate(
                r#"(() => {
                    const style = document.createElement('style');
                    style.id = '__rayo_freeze_animations';
                    style.textContent = '*, *::before, *::after { animation-duration: 0s !important; animation-delay: 0s !important; transition-duration: 0s !important; transition-delay: 0s !important; }';
                    document.head.appendChild(style);
                })()"#,
            )
            .await?;
        Ok(())
    }

    /// Remove the animation freeze CSS injection.
    pub async fn unfreeze_animations(&self) -> Result<(), RayoError> {
        self.page
            .evaluate(
                r#"(() => {
                    const el = document.getElementById('__rayo_freeze_animations');
                    if (el) el.remove();
                })()"#,
            )
            .await?;
        Ok(())
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

    /// Hover over an element by selector or page map ID.
    /// Uses CDP Input.dispatchMouseEvent via chromiumoxide for real mouse events.
    /// Useful for triggering dropdown menus and tooltips.
    pub async fn hover(&self, selector: Option<&str>, id: Option<usize>) -> Result<(), RayoError> {
        self.hover_raw(selector, id).await?;
        self.invalidate_after_mutation().await;
        Ok(())
    }

    /// Internal hover without cache invalidation — used by batch executor
    /// to defer all invalidation to a single pass at the end.
    async fn hover_raw(&self, selector: Option<&str>, id: Option<usize>) -> Result<(), RayoError> {
        let sel = self.resolve_selector(selector, id).await?;
        let _span = self.profiler.start_span(
            format!("hover({})", truncate(&sel, 40)),
            SpanCategory::DomMutate,
        );
        // Use CDP Input events via chromiumoxide Element API
        // Element::hover() internally calls scroll_into_view() + move_mouse()
        let element =
            self.page
                .find_element(&sel)
                .await
                .map_err(|e| RayoError::ElementNotFound {
                    selector: format!("{sel}: {e}"),
                })?;
        element
            .hover()
            .await
            .map_err(|e| RayoError::Cdp(format!("hover failed: {e}")))?;
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
        self.type_text_raw(selector, id, text, clear).await?;
        self.invalidate_after_mutation().await;
        Ok(())
    }

    /// Internal type_text without cache invalidation — used by batch executor
    /// to defer all invalidation to a single pass at the end.
    async fn type_text_raw(
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
        self.press_key_raw(selector, id, key).await?;
        self.invalidate_after_mutation().await;
        Ok(())
    }

    /// Internal press_key without cache invalidation — used by batch executor
    /// to defer all invalidation to a single pass at the end.
    async fn press_key_raw(
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
        Ok(())
    }

    /// Select an option from a dropdown.
    pub async fn select_option(
        &self,
        selector: Option<&str>,
        id: Option<usize>,
        value: &str,
    ) -> Result<(), RayoError> {
        self.select_option_raw(selector, id, value).await?;
        self.invalidate_after_mutation().await;
        Ok(())
    }

    /// Internal select_option without cache invalidation — used by batch executor
    /// to defer all invalidation to a single pass at the end.
    async fn select_option_raw(
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
                    self.type_text_raw(sel, id, value, true).await.map(|_| None)
                }
                BatchAction::Select { target, value } => {
                    let (sel, id) = target_to_selector_id(target);
                    self.select_option_raw(sel, id, value).await.map(|_| None)
                }
                BatchAction::Goto { url } => self.goto_raw(url).await.map(|_| None),
                BatchAction::Screenshot { full_page } => self
                    .screenshot(*full_page)
                    .await
                    .map(|b64| Some(serde_json::Value::String(b64))),
                BatchAction::WaitFor {
                    target,
                    timeout_ms,
                    visible,
                } => {
                    let (sel, id) = target_to_selector_id(target);
                    let selector = self.resolve_selector(sel, id).await?;
                    self.wait_for_selector(&selector, *timeout_ms, visible.unwrap_or(false))
                        .await
                        .map(|_| None)
                }
                BatchAction::Press { target, key } => {
                    let (sel, id) = if let Some(t) = target {
                        target_to_selector_id(t)
                    } else {
                        (None, None)
                    };
                    self.press_key_raw(sel, id, key).await.map(|_| None)
                }
                BatchAction::Hover { target } => {
                    let (sel, id) = target_to_selector_id(target);
                    self.hover_raw(sel, id).await.map(|_| None)
                }
                BatchAction::Inspect { target, compact } => {
                    let (sel, id) = target_to_selector_id(target);
                    let options = InspectOptions {
                        compact: *compact,
                        ..Default::default()
                    };
                    let result = self.inspect_element(sel, id, &options).await?;
                    Ok(Some(serde_json::to_value(&result).unwrap_or_default()))
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
    ///
    /// When `visible` is true, also checks that the element is visible (has layout
    /// dimensions or a non-null offsetParent), not just present in the DOM.
    pub async fn wait_for_selector(
        &self,
        selector: &str,
        timeout_ms: u64,
        visible: bool,
    ) -> Result<(), RayoError> {
        let _span = self.profiler.start_span(
            format!("wait({})", truncate(selector, 40)),
            SpanCategory::Wait,
        );
        let sel_json = serde_json::to_string(selector).unwrap();
        let visible_js = if visible {
            "function isVisible(el) { return el.offsetParent !== null || el.offsetWidth > 0 || el.offsetHeight > 0; }"
        } else {
            "function isVisible() { return true; }"
        };
        let js = format!(
            r#"new Promise((resolve, reject) => {{
                {visible_js}
                const sel = {sel_json};
                const el = document.querySelector(sel);
                if (el && isVisible(el)) {{ resolve(true); return; }}
                const observer = new MutationObserver(() => {{
                    const found = document.querySelector(sel);
                    if (found && isVisible(found)) {{
                        observer.disconnect();
                        resolve(true);
                    }}
                }});
                observer.observe(document.body || document.documentElement, {{ childList: true, subtree: true, attributes: {visible_check} }});
                setTimeout(() => {{
                    observer.disconnect();
                    reject(new Error('timeout'));
                }}, {timeout_ms});
            }})"#,
            visible_check = if visible { "true" } else { "false" },
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

    /// Wait for network idle: no new network requests for `quiet_ms` milliseconds.
    /// Times out after `timeout_ms` if network never goes idle.
    ///
    /// Uses the Performance API to detect pending resource fetches,
    /// polling every 100ms until the quiet period is achieved.
    pub async fn wait_for_network_idle(
        &self,
        quiet_ms: u64,
        timeout_ms: u64,
    ) -> Result<(), RayoError> {
        let _span = self.profiler.start_span(
            format!("wait_network_idle({}ms)", quiet_ms),
            SpanCategory::Wait,
        );

        let js = format!(
            r#"new Promise((resolve, reject) => {{
                let lastActivity = Date.now();
                const quietMs = {quiet_ms};
                const timeoutMs = {timeout_ms};
                const startTime = Date.now();

                // Track ongoing fetches via PerformanceObserver
                const observer = new PerformanceObserver((list) => {{
                    for (const entry of list.getEntries()) {{
                        lastActivity = Date.now();
                    }}
                }});
                try {{
                    observer.observe({{ type: 'resource', buffered: false }});
                }} catch (e) {{
                    // PerformanceObserver not supported — fall back to simple timeout
                    setTimeout(() => resolve(true), quietMs);
                    return;
                }}

                const check = setInterval(() => {{
                    const now = Date.now();
                    if (now - lastActivity >= quietMs) {{
                        clearInterval(check);
                        observer.disconnect();
                        resolve(true);
                    }} else if (now - startTime >= timeoutMs) {{
                        clearInterval(check);
                        observer.disconnect();
                        resolve(true); // resolve anyway on timeout — best effort
                    }}
                }}, 100);
            }})"#,
        );

        self.page
            .evaluate(js)
            .await
            .map_err(|e| RayoError::Cdp(format!("wait_for_network_idle failed: {e}")))?;
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
    pub async fn set_cookies(&self, cookies: Vec<SetCookie>) -> Result<CookieSetResult, RayoError> {
        let _span = self.profiler.start_span(
            format!("set_cookies({})", cookies.len()),
            SpanCategory::CdpCommand,
        );
        let mut result = CookieSetResult {
            set: 0,
            failed: Vec::new(),
        };
        for cookie in cookies {
            let name = cookie.name.clone();
            let cdp = to_cdp_cookie(cookie);
            match self.page.set_cookie(cdp).await {
                Ok(_) => result.set += 1,
                Err(e) => {
                    tracing::warn!("Failed to set cookie '{name}': {e}");
                    result.failed.push(name);
                }
            }
        }
        tracing::debug!(
            "Set {}/{} cookies",
            result.set,
            result.set + result.failed.len()
        );
        if result.set == 0 && !result.failed.is_empty() {
            return Err(RayoError::CookieError(format!(
                "Failed to set any cookies. Rejected: {:?}",
                result.failed
            )));
        }
        Ok(result)
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
                            .filter_map(|(k, v)| v.as_str().map(|val| (k.clone(), val.to_string())))
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
                            .filter_map(|(k, v)| v.as_str().map(|val| (k.clone(), val.to_string())))
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

                    let mut params = FulfillRequestParams::new(request_id, mock.status as i64);
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

                if let Err(e) = page.execute(ContinueRequestParams::new(request_id)).await {
                    tracing::warn!("Fetch.continueRequest failed: {e}");
                }
            }
        });

        Ok(())
    }

    /// Inspect an element's computed styles, box model, applied rules, and diagnostics.
    ///
    /// This is the "DevTools Elements panel" for AI agents. Uses CDP CSS domain
    /// for applied rules + computed styles, and JS evaluation for visibility/anomaly
    /// diagnostics. All CDP calls run in parallel via tokio::try_join!.
    pub async fn inspect_element(
        &self,
        selector: Option<&str>,
        id: Option<usize>,
        options: &InspectOptions,
    ) -> Result<InspectResult, RayoError> {
        let _span = self
            .profiler
            .start_span("inspect_element", SpanCategory::Inspect);

        // Resolve the element selector
        let resolved_selector = self.resolve_selector(selector, id).await?;

        // Enable CSS domain once per session
        if !self.css_enabled.load(Ordering::Relaxed) {
            let _ = self.page.execute(CssEnableParams::default()).await;
            self.css_enabled.store(true, Ordering::Relaxed);
        }

        // Get the DOM document root, then querySelector to get nodeId
        let doc = self
            .page
            .execute(GetDocumentParams::builder().depth(0).build())
            .await
            .map_err(|e| RayoError::Cdp(format!("DOM.getDocument: {e}")))?;
        let root_node_id = doc.result.root.node_id;

        let qs_result = self
            .page
            .execute(QuerySelectorParams::new(root_node_id, &resolved_selector))
            .await
            .map_err(|e| RayoError::ElementNotFound {
                selector: format!("{resolved_selector} (CDP querySelector failed: {e})"),
            })?;
        let node_id = qs_result.result.node_id;

        // Parallel CDP calls: computed styles, matched rules, box model
        // Plus JS diagnostics — all independent, fan out with tokio::try_join!
        let computed_fut = self
            .page
            .execute(GetComputedStyleForNodeParams::new(node_id));
        let matched_fut = self
            .page
            .execute(GetMatchedStylesForNodeParams::new(node_id));
        let box_model_fut = self
            .page
            .execute(GetBoxModelParams::builder().node_id(node_id).build());
        let diag_js = format!(
            "{}({})",
            inspect::INSPECT_DIAGNOSTICS_JS,
            serde_json::to_string(&resolved_selector).unwrap_or_default()
        );
        let diag_fut = self.page.evaluate(diag_js);

        let (computed_res, matched_res, box_model_res, diag_res) =
            tokio::join!(computed_fut, matched_fut, box_model_fut, diag_fut);

        // Process computed styles
        let mut computed_map = std::collections::HashMap::new();
        let mut all_vars = std::collections::HashMap::new();
        if let Ok(computed) = &computed_res {
            for prop in &computed.result.computed_style {
                computed_map.insert(prop.name.clone(), prop.value.clone());
                if prop.name.starts_with("--") {
                    all_vars.insert(prop.name.clone(), prop.value.clone());
                }
            }
        }

        // Filter properties based on options
        let filtered_computed = if options.all {
            Some(computed_map.clone())
        } else if let Some(ref props) = options.properties {
            let resolved_props = inspect::resolve_properties(props);
            let filtered: std::collections::HashMap<_, _> = computed_map
                .iter()
                .filter(|(k, _)| resolved_props.contains(k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            Some(filtered)
        } else {
            // Curated default
            let filtered: std::collections::HashMap<_, _> = computed_map
                .iter()
                .filter(|(k, _)| inspect::CURATED_PROPERTIES.contains(&k.as_str()))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            Some(filtered)
        };

        // Process applied rules (graceful degradation)
        let applied_rules = if let Ok(matched) = &matched_res {
            let mut rules = Vec::new();
            if let Some(ref rule_matches) = matched.result.matched_css_rules {
                for rm in rule_matches {
                    let selector_text = rm.rule.selector_list.text.clone();
                    let mut properties = std::collections::HashMap::new();
                    for prop in &rm.rule.style.css_properties {
                        if !prop.name.is_empty()
                            && !prop.value.is_empty()
                            && !prop.disabled.unwrap_or(false)
                        {
                            properties.insert(prop.name.clone(), prop.value.clone());
                        }
                    }
                    if !properties.is_empty() {
                        let source = rm
                            .rule
                            .style
                            .range
                            .as_ref()
                            .map(|r| format!("line {}", r.start_line));
                        rules.push(AppliedRule {
                            selector: selector_text,
                            source,
                            properties,
                            specificity: None,
                        });
                    }
                }
            }
            // Include inline styles
            if let Some(ref inline) = matched.result.inline_style {
                let mut properties = std::collections::HashMap::new();
                for prop in &inline.css_properties {
                    if !prop.name.is_empty() && !prop.value.is_empty() {
                        properties.insert(prop.name.clone(), prop.value.clone());
                    }
                }
                if !properties.is_empty() {
                    rules.insert(
                        0,
                        AppliedRule {
                            selector: "[inline]".into(),
                            source: Some("inline style".into()),
                            properties,
                            specificity: Some([1, 0, 0]),
                        },
                    );
                }
            }
            if rules.is_empty() { None } else { Some(rules) }
        } else {
            None
        };

        // Process box model (graceful degradation)
        let box_model = if let Ok(bm) = &box_model_res {
            let m = &bm.result.model;
            Some(inspect::BoxModel {
                content: [m.width as f64, m.height as f64],
                padding: quad_to_dimensions(&m.padding, &m.content),
                border: quad_to_dimensions(&m.border, &m.padding),
                margin: quad_to_dimensions(&m.margin, &m.border),
            })
        } else {
            None
        };

        // Process JS diagnostics (visibility, anomalies)
        let (visibility, anomalies, tag) = if let Ok(diag_val) = diag_res {
            let diag: serde_json::Value = diag_val.into_value().unwrap_or_default();
            let vis = diag.get("visibility").map(|v| VisibilityInfo {
                visible: v.get("visible").and_then(|b| b.as_bool()).unwrap_or(true),
                diagnosis: v
                    .get("diagnosis")
                    .and_then(|d| d.as_str())
                    .map(String::from),
            });
            let anomalies: Vec<String> = diag
                .get("anomalies")
                .and_then(|a| a.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let tag = diag
                .get("tag")
                .and_then(|t| t.as_str())
                .unwrap_or("unknown")
                .to_string();
            (vis, anomalies, tag)
        } else {
            (None, vec![], "unknown".to_string())
        };

        // Resolve CSS variable chains
        let variables = if !all_vars.is_empty() {
            // Collect properties that reference variables from matched rules
            let mut matched_vars = std::collections::HashMap::new();
            if let Some(ref rules) = applied_rules {
                for rule in rules {
                    for (name, value) in &rule.properties {
                        if value.contains("var(") {
                            matched_vars.insert(name.clone(), value.clone());
                        }
                    }
                }
            }
            let chains = inspect::resolve_variable_chains(&matched_vars, &all_vars);
            if chains.is_empty() {
                None
            } else {
                Some(chains)
            }
        } else {
            None
        };

        // Accessibility info (via JS since CDP a11y domain is heavyweight)
        let a11y = if !options.compact {
            let a11y_js = format!(
                r#"(() => {{
                    const el = document.querySelector({});
                    if (!el) return null;
                    return {{
                        role: el.getAttribute('role') || el.tagName.toLowerCase(),
                        name: el.getAttribute('aria-label') || el.textContent?.trim()?.slice(0, 100) || null,
                        focusable: el.tabIndex >= 0,
                        states: [
                            el.getAttribute('aria-expanded') ? 'expanded=' + el.getAttribute('aria-expanded') : null,
                            el.getAttribute('aria-disabled') === 'true' ? 'disabled' : null,
                            el.getAttribute('aria-hidden') === 'true' ? 'hidden' : null,
                            el.getAttribute('aria-selected') === 'true' ? 'selected' : null,
                        ].filter(Boolean),
                    }};
                }})()"#,
                serde_json::to_string(&resolved_selector).unwrap_or_default()
            );
            if let Ok(a11y_val) = self.page.evaluate(a11y_js).await {
                let v: serde_json::Value = a11y_val.into_value().unwrap_or_default();
                if v.is_object() {
                    Some(AccessibilityInfo {
                        role: v.get("role").and_then(|r| r.as_str()).map(String::from),
                        name: v.get("name").and_then(|n| n.as_str()).map(String::from),
                        focusable: v
                            .get("focusable")
                            .and_then(|f| f.as_bool())
                            .unwrap_or(false),
                        states: v
                            .get("states")
                            .and_then(|s| s.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|s| s.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // Style diff (opt-in)
        let diff = if options.diff {
            let mut dc = self.diff_cache.lock().await;
            let prev = dc.get(&resolved_selector).cloned();
            if let Some(prev_styles) = prev {
                let mut changed = std::collections::HashMap::new();
                for (k, v) in &computed_map {
                    if let Some(prev_v) = prev_styles.get(k)
                        && prev_v != v
                    {
                        changed.insert(
                            k.clone(),
                            StyleChange {
                                before: prev_v.clone(),
                                after: v.clone(),
                            },
                        );
                    }
                }
                if changed.is_empty() {
                    None
                } else {
                    Some(StyleDiff { changed })
                }
            } else {
                None
            }
        } else {
            None
        };

        // Always cache current computed styles for future diffs
        self.diff_cache
            .lock()
            .await
            .put(resolved_selector.clone(), computed_map.clone());

        // Expect assertions (opt-in)
        let expect_results = if let Some(ref expectations) = options.expect {
            let results: Vec<inspect::ExpectResult> = expectations
                .iter()
                .map(|(prop, expected)| {
                    let actual = computed_map.get(prop).cloned().unwrap_or_default();
                    let pass = actual == *expected;
                    inspect::ExpectResult {
                        property: prop.clone(),
                        expected: expected.clone(),
                        actual,
                        pass,
                    }
                })
                .collect();
            Some(results)
        } else {
            None
        };

        // Build summary
        let vis_str = visibility
            .as_ref()
            .map(|v| if v.visible { "visible" } else { "hidden" })
            .unwrap_or("unknown");
        let dims = box_model
            .as_ref()
            .map(|b| format!("{}x{}", b.content[0] as i64, b.content[1] as i64))
            .unwrap_or_else(|| "?x?".into());
        let bg = computed_map
            .get("background-color")
            .map(|v| format!(", bg: {}", v))
            .unwrap_or_default();
        let anomaly_str = if anomalies.is_empty() {
            ", no anomalies".into()
        } else {
            format!(", {} anomalies", anomalies.len())
        };
        let summary = format!(
            "{}.{}: {vis_str}, {dims}{bg}{anomaly_str}",
            tag, resolved_selector
        );

        // Collect warnings
        let mut warnings = Vec::new();
        if computed_res.is_err() {
            warnings.push("computed styles unavailable".into());
        }
        if matched_res.is_err() {
            warnings.push("applied rules unavailable (CSS domain error)".into());
        }
        if box_model_res.is_err() {
            warnings.push("box model unavailable (element may be display:none)".into());
        }

        // Compact mode: strip heavy sections
        let result = if options.compact {
            InspectResult {
                target: InspectTarget {
                    selector: resolved_selector,
                    tag,
                    id,
                },
                summary,
                anomalies,
                visibility,
                computed: filtered_computed,
                box_model,
                applied_rules: None,
                variables: None,
                accessibility: None,
                diff,
                expect_results,
                warnings,
            }
        } else {
            InspectResult {
                target: InspectTarget {
                    selector: resolved_selector,
                    tag,
                    id,
                },
                summary,
                anomalies,
                visibility,
                computed: filtered_computed,
                box_model,
                applied_rules,
                variables,
                accessibility: a11y,
                diff,
                expect_results,
                warnings,
            }
        };

        Ok(result)
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

/// Convert CDP Quad (8 points: 4 corners x,y) to dimension offsets [top, right, bottom, left]
/// relative to an inner quad.
fn quad_to_dimensions(
    outer: &chromiumoxide::cdp::browser_protocol::dom::Quad,
    inner: &chromiumoxide::cdp::browser_protocol::dom::Quad,
) -> [f64; 4] {
    // Quad is [x1,y1, x2,y2, x3,y3, x4,y4] for top-left, top-right, bottom-right, bottom-left
    let outer_pts = &outer.inner();
    let inner_pts = &inner.inner();
    if outer_pts.len() >= 8 && inner_pts.len() >= 8 {
        [
            (inner_pts[1] - outer_pts[1]).abs(), // top
            (outer_pts[2] - inner_pts[2]).abs(), // right
            (outer_pts[5] - inner_pts[5]).abs(), // bottom
            (inner_pts[0] - outer_pts[0]).abs(), // left
        ]
    } else {
        [0.0; 4]
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
        BatchAction::Hover { .. } => "hover",
        BatchAction::Inspect { .. } => "inspect",
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
    let mut cp = CookieParam::new(c.name.clone(), c.value);

    // Synthesize URL from domain for CDP (required for cookie injection).
    let url = c.url.clone().or_else(|| {
        c.domain.as_ref().map(|d| {
            let d = d.trim_start_matches('.');
            let scheme = if c.secure == Some(true) {
                "https"
            } else {
                "http"
            };
            format!("{scheme}://{d}/")
        })
    });
    cp.url = url;

    // __Host- prefix cookies must NOT have a domain attribute — CDP rejects them.
    // For other cookies, set domain only if there's no URL (prefer URL over domain).
    if c.name.starts_with("__Host-") {
        cp.domain = None;
    } else {
        cp.domain = c.domain;
    }

    cp.path = c.path;
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
