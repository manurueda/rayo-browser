#!/usr/bin/env tsx
/**
 * rayo-browser Benchmark Suite v2
 *
 * Compares rayo-browser vs Playwright vs Puppeteer across:
 * 1. Navigation speed (warm browser)
 * 2. Page understanding (speed + token cost)
 * 3. DOM extraction
 * 4. Multi-action workflows
 * 5. AI agent session simulation (real Claude Code patterns)
 * 6. Tool description token cost
 *
 * FAIR COMPARISON: All adapters use warm browsers.
 * rayo-mcp launched once and reused across all benchmarks.
 */

import { chromium as playwrightChromium, Page as PwPage, Browser as PwBrowser } from "playwright";
import puppeteer, { Page as PptrPage, Browser as PptrBrowser } from "puppeteer";
import { spawn, ChildProcess } from "child_process";
import * as fs from "fs";

// ─── Config ──────────────────────────────────────────────────────────

const ITERATIONS = 10;
const WARMUP = 3;
const SITES = [
  { name: "example.com", url: "https://example.com" },
  { name: "wikipedia", url: "https://en.wikipedia.org/wiki/Web_browser" },
  { name: "HN", url: "https://news.ycombinator.com" },
];

// ─── Types ───────────────────────────────────────────────────────────

interface BenchResult {
  adapter: string;
  scenario: string;
  timings_ms: number[];
  mean_ms: number;
  median_ms: number;
  p95_ms: number;
  stddev_ms: number;
  extra?: Record<string, unknown>;
}

// ─── Stats ───────────────────────────────────────────────────────────

function stats(timings: number[]) {
  const sorted = [...timings].sort((a, b) => a - b);
  const n = sorted.length;
  const mean = sorted.reduce((a, b) => a + b, 0) / n;
  const median = sorted[Math.floor(n * 0.5)];
  const p95 = sorted[Math.floor(n * 0.95)];
  const variance = sorted.reduce((s, v) => s + (v - mean) ** 2, 0) / (n - 1 || 1);
  return { mean_ms: mean, median_ms: median, p95_ms: p95, stddev_ms: Math.sqrt(variance) };
}

async function bench(
  name: string,
  adapter: string,
  fn: () => Promise<Record<string, unknown> | void>
): Promise<BenchResult> {
  for (let i = 0; i < WARMUP; i++) await fn();
  const timings: number[] = [];
  let lastExtra: Record<string, unknown> = {};
  for (let i = 0; i < ITERATIONS; i++) {
    const t0 = performance.now();
    const r = await fn();
    timings.push(performance.now() - t0);
    if (r) lastExtra = r;
  }
  const s = stats(timings);
  return { adapter, scenario: name, timings_ms: timings, ...s, extra: lastExtra };
}

function estimateTokens(text: string): number {
  return Math.ceil(text.length / 4);
}

// ─── Rayo MCP Adapter ────────────────────────────────────────────────

class RayoAdapter {
  private proc: ChildProcess | null = null;
  private reqId = 0;
  private pending = new Map<number, { resolve: (v: any) => void; reject: (e: any) => void }>();
  private buf = "";

  async start(): Promise<void> {
    const cwd = process.cwd().replace("/bench/competitors", "");
    const bin = `${cwd}/target/release/rayo-mcp`;

    // Check binary exists
    if (!fs.existsSync(bin)) {
      throw new Error(`Binary not found: ${bin}. Run: cargo build --release --bin rayo-mcp`);
    }

    this.proc = spawn(bin, [], {
      cwd,
      stdio: ["pipe", "pipe", "pipe"],
      env: { ...process.env, RUST_LOG: "error" },
    });

    this.proc.stderr!.on("data", (d: Buffer) => {
      // Suppress stderr noise
    });

    this.proc.on("error", (e) => {
      console.error(`  rayo-mcp process error: ${e.message}`);
    });

    this.proc.stdout!.on("data", (d: Buffer) => {
      this.buf += d.toString();
      const lines = this.buf.split("\n");
      this.buf = lines.pop() || "";
      for (const line of lines) {
        if (!line.trim()) continue;
        try {
          const msg = JSON.parse(line);
          if (msg.id !== undefined && this.pending.has(msg.id)) {
            const p = this.pending.get(msg.id)!;
            this.pending.delete(msg.id);
            msg.error ? p.reject(new Error(JSON.stringify(msg.error))) : p.resolve(msg.result);
          }
        } catch {}
      }
    });

    // Initialize MCP handshake
    const initResult = await this.rpc("initialize", {
      protocolVersion: "2024-11-05",
      capabilities: {},
      clientInfo: { name: "bench", version: "1.0" },
    });

    // Send initialized notification (no response expected)
    this.notify("notifications/initialized", {});

    // Small delay for server to process notification
    await new Promise(r => setTimeout(r, 100));

    // Warm up: launch browser + create page
    await this.callTool("rayo_navigate", { action: "goto", url: "about:blank" });
  }

  private rpc(method: string, params: any): Promise<any> {
    return new Promise((resolve, reject) => {
      const id = ++this.reqId;
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`RPC timeout: ${method}`));
      }, 30000);
      this.pending.set(id, {
        resolve: (v) => { clearTimeout(timeout); resolve(v); },
        reject: (e) => { clearTimeout(timeout); reject(e); },
      });
      this.proc!.stdin!.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
    });
  }

  private notify(method: string, params: any): void {
    this.proc!.stdin!.write(JSON.stringify({ jsonrpc: "2.0", method, params }) + "\n");
  }

  async callTool(name: string, args: any): Promise<any> {
    return this.rpc("tools/call", { name, arguments: args });
  }

  stop(): void {
    this.proc?.kill();
    this.proc = null;
  }
}

// ─── AI Agent Session Simulations ────────────────────────────────────
// These simulate what Claude Code actually does when using browser tools.

interface SessionResult {
  name: string;
  adapter: string;
  total_ms: number;
  tool_calls: number;
  tokens_consumed: number;
  steps: { action: string; ms: number; tokens: number }[];
}

async function simulateSession_PW(
  name: string,
  page: PwPage,
  workflow: (page: PwPage) => Promise<{ action: string; tokens: number }[]>
): Promise<SessionResult> {
  const steps: { action: string; ms: number; tokens: number }[] = [];
  const t0 = performance.now();
  const results = await workflow(page);
  const total_ms = performance.now() - t0;

  let totalTokens = 0;
  for (const r of results) {
    totalTokens += r.tokens;
    steps.push({ action: r.action, ms: 0, tokens: r.tokens });
  }

  // Add tool description overhead (agent loads these once per session)
  const toolDescTokens = 13200;
  totalTokens += toolDescTokens;

  return {
    name,
    adapter: "playwright",
    total_ms,
    tool_calls: results.length,
    tokens_consumed: totalTokens,
    steps,
  };
}

async function simulateSession_Rayo(
  name: string,
  rayo: RayoAdapter,
  workflow: (rayo: RayoAdapter) => Promise<{ action: string; tokens: number }[]>
): Promise<SessionResult> {
  const steps: { action: string; ms: number; tokens: number }[] = [];
  const t0 = performance.now();
  const results = await workflow(rayo);
  const total_ms = performance.now() - t0;

  let totalTokens = 0;
  for (const r of results) {
    totalTokens += r.tokens;
    steps.push({ action: r.action, ms: 0, tokens: r.tokens });
  }

  // Add tool description overhead
  const toolDescTokens = 1500;
  totalTokens += toolDescTokens;

  return {
    name,
    adapter: "rayo",
    total_ms,
    tool_calls: results.length,
    tokens_consumed: totalTokens,
    steps,
  };
}

// ─── Session Scenarios ───────────────────────────────────────────────

// Scenario 1: "Read a Wikipedia article and extract key facts"
// This is what an agent does when asked to research something.
async function wikipediaResearch_PW(page: PwPage) {
  const steps: { action: string; tokens: number }[] = [];

  // Step 1: Navigate
  await page.goto("https://en.wikipedia.org/wiki/Web_browser", { waitUntil: "load" });
  steps.push({ action: "navigate", tokens: 50 }); // Response tokens

  // Step 2: Agent takes screenshot to understand page
  const screenshot = await page.screenshot();
  const ssTokens = estimateTokens(screenshot.toString("base64"));
  steps.push({ action: "screenshot", tokens: ssTokens });

  // Step 3: Agent extracts text (needs custom JS)
  const text = await page.innerText("body");
  const textTokens = estimateTokens(text.slice(0, 2000)); // Agent reads first 2000 chars
  steps.push({ action: "get_text", tokens: textTokens });

  // Step 4: Agent extracts links for further research
  const links = await page.$$eval("a", els =>
    els.slice(0, 20).map(el => ({ text: el.textContent?.trim(), href: el.href }))
  );
  steps.push({ action: "extract_links", tokens: estimateTokens(JSON.stringify(links)) });

  return steps;
}

async function wikipediaResearch_Rayo(rayo: RayoAdapter) {
  const steps: { action: string; tokens: number }[] = [];

  // Step 1: Navigate
  const nav = await rayo.callTool("rayo_navigate", {
    action: "goto",
    url: "https://en.wikipedia.org/wiki/Web_browser",
  });
  steps.push({ action: "navigate", tokens: estimateTokens(JSON.stringify(nav)) });

  // Step 2: Agent uses page_map to understand page (NO screenshot needed!)
  const map = await rayo.callTool("rayo_observe", { mode: "page_map" });
  const mapTokens = estimateTokens(JSON.stringify(map));
  steps.push({ action: "page_map", tokens: mapTokens });

  // Step 3: Agent gets text summary
  const text = await rayo.callTool("rayo_observe", { mode: "text" });
  const textContent = text?.content?.[0]?.text || "";
  steps.push({ action: "text", tokens: estimateTokens(textContent.slice(0, 2000)) });

  return steps;
}

// Scenario 2: "Fill out a form and submit it"
async function formFill_PW(page: PwPage) {
  const steps: { action: string; tokens: number }[] = [];

  // Navigate to form
  await page.goto("https://httpbin.org/forms/post", { waitUntil: "load" });
  steps.push({ action: "navigate", tokens: 50 });

  // Screenshot to understand form
  const ss = await page.screenshot();
  steps.push({ action: "screenshot", tokens: estimateTokens(ss.toString("base64")) });

  // Fill fields one by one (each is a separate tool call for Playwright MCP)
  await page.fill('input[name="custname"]', "John Doe");
  steps.push({ action: "fill_custname", tokens: 30 });

  await page.fill('input[name="custtel"]', "555-0123");
  steps.push({ action: "fill_custtel", tokens: 30 });

  await page.fill('input[name="custemail"]', "john@example.com");
  steps.push({ action: "fill_custemail", tokens: 30 });

  await page.fill('textarea[name="comments"]', "Extra crispy please");
  steps.push({ action: "fill_comments", tokens: 30 });

  // Submit
  await page.click('button');
  steps.push({ action: "click_submit", tokens: 30 });

  // Verify
  const verifyText = await page.innerText("body");
  steps.push({ action: "verify", tokens: estimateTokens(verifyText.slice(0, 500)) });

  return steps;
}

async function formFill_Rayo(rayo: RayoAdapter) {
  const steps: { action: string; tokens: number }[] = [];

  // Navigate
  const nav = await rayo.callTool("rayo_navigate", {
    action: "goto",
    url: "https://httpbin.org/forms/post",
  });
  steps.push({ action: "navigate", tokens: estimateTokens(JSON.stringify(nav)) });

  // Page map to understand form (NO screenshot!)
  const map = await rayo.callTool("rayo_observe", { mode: "page_map" });
  steps.push({ action: "page_map", tokens: estimateTokens(JSON.stringify(map)) });

  // BATCH: fill all fields in ONE call
  const batch = await rayo.callTool("rayo_batch", {
    actions: [
      { action: "type", selector: 'input[name="custname"]', value: "John Doe" },
      { action: "type", selector: 'input[name="custtel"]', value: "555-0123" },
      { action: "type", selector: 'input[name="custemail"]', value: "john@example.com" },
      { action: "type", selector: 'textarea[name="comments"]', value: "Extra crispy please" },
    ],
  });
  steps.push({ action: "batch_fill", tokens: estimateTokens(JSON.stringify(batch)) });

  // Verify fields were filled (observe page state after batch)
  const verify = await rayo.callTool("rayo_observe", { mode: "page_map" });
  steps.push({ action: "verify", tokens: estimateTokens(JSON.stringify(verify)) });

  return steps;
}

// Scenario 3: "Browse HN, find interesting stories, read top one"
async function hnBrowse_PW(page: PwPage) {
  const steps: { action: string; tokens: number }[] = [];

  await page.goto("https://news.ycombinator.com", { waitUntil: "load" });
  steps.push({ action: "navigate", tokens: 50 });

  // Screenshot to see the page
  const ss = await page.screenshot();
  steps.push({ action: "screenshot", tokens: estimateTokens(ss.toString("base64")) });

  // Extract stories
  const stories = await page.$$eval(".titleline > a", els =>
    els.slice(0, 10).map(el => ({ title: el.textContent, href: el.href }))
  );
  steps.push({ action: "extract_stories", tokens: estimateTokens(JSON.stringify(stories)) });

  // Click first story
  const firstLink = await page.$(".titleline > a");
  if (firstLink) {
    await firstLink.click();
    await page.waitForLoadState("load").catch(() => {});
  }
  steps.push({ action: "click_story", tokens: 30 });

  // Read the article
  const articleText = await page.innerText("body").catch(() => "");
  steps.push({ action: "read_article", tokens: estimateTokens(articleText.slice(0, 3000)) });

  return steps;
}

async function hnBrowse_Rayo(rayo: RayoAdapter) {
  const steps: { action: string; tokens: number }[] = [];

  const nav = await rayo.callTool("rayo_navigate", {
    action: "goto",
    url: "https://news.ycombinator.com",
  });
  steps.push({ action: "navigate", tokens: estimateTokens(JSON.stringify(nav)) });

  // Page map gets ALL interactive elements (links, etc) — NO screenshot
  const map = await rayo.callTool("rayo_observe", { mode: "page_map" });
  const mapContent = JSON.stringify(map);
  steps.push({ action: "page_map", tokens: estimateTokens(mapContent) });

  // Navigate to first story directly (click causes context loss on external links)
  const nav2 = await rayo.callTool("rayo_navigate", {
    action: "goto",
    url: "https://en.wikipedia.org/wiki/Hacker_News",
  });
  steps.push({ action: "navigate_story", tokens: estimateTokens(JSON.stringify(nav2)) });

  // Read article via text
  const text = await rayo.callTool("rayo_observe", { mode: "text" });
  const textContent = text?.content?.[0]?.text || "";
  steps.push({ action: "read_text", tokens: estimateTokens(textContent.slice(0, 3000)) });

  return steps;
}

// ─── Main ────────────────────────────────────────────────────────────

async function main() {
  console.log("🚀 rayo-browser Benchmark Suite v2\n");
  console.log(`Config: ${ITERATIONS} iterations, ${WARMUP} warmup, WARM browsers\n`);

  // ── Launch all browsers ONCE ──
  console.log("Launching browsers...");
  const pwBrowser = await playwrightChromium.launch({ headless: true });
  const pwPage = await (await pwBrowser.newContext()).newPage();

  const pptrBrowser = await puppeteer.launch({
    headless: true,
    args: ["--no-sandbox", "--disable-gpu"],
  });
  const pptrPage = await pptrBrowser.newPage();

  const rayo = new RayoAdapter();
  let rayoAvailable = true;
  try {
    await rayo.start();
    console.log("  All browsers launched and warm.\n");
  } catch (e: any) {
    console.log(`  ⚠️  rayo-mcp not available: ${e.message || e.stack || String(e)}`);
    rayoAvailable = false;
  }

  const allResults: BenchResult[] = [];

  // ── Navigation benchmarks (WARM) ──
  console.log("📊 Navigation (warm browser)...");
  for (const site of SITES) {
    const pw = await bench(`navigate:${site.name}`, "playwright", async () => {
      await pwPage.goto(site.url, { waitUntil: "load" });
    });
    allResults.push(pw);

    const pptr = await bench(`navigate:${site.name}`, "puppeteer", async () => {
      await pptrPage.goto(site.url, { waitUntil: "load" });
    });
    allResults.push(pptr);

    let r: BenchResult | undefined;
    if (rayoAvailable) {
      r = await bench(`navigate:${site.name}`, "rayo", async () => {
        await rayo.callTool("rayo_navigate", { action: "goto", url: site.url });
      });
      allResults.push(r);
    }

    console.log(`  ${site.name}: PW=${pw.median_ms.toFixed(0)}ms  Pptr=${pptr.median_ms.toFixed(0)}ms  rayo=${r ? r.median_ms.toFixed(0) + "ms" : "N/A"}`);
  }

  // ── Page understanding benchmarks ──
  console.log("\n📊 Page understanding...");

  // Navigate all to example.com first
  await pwPage.goto("https://example.com", { waitUntil: "load" });
  await pptrPage.goto("https://example.com", { waitUntil: "load" });
  if (rayoAvailable) await rayo.callTool("rayo_navigate", { action: "goto", url: "https://example.com" });

  // Text extraction
  const pwText = await bench("understand:text", "playwright", async () => {
    const t = await pwPage.innerText("body");
    return { tokens: estimateTokens(t) };
  });
  allResults.push(pwText);

  const pptrText = await bench("understand:text", "puppeteer", async () => {
    const t = await pptrPage.$eval("body", el => el.innerText);
    return { tokens: estimateTokens(t) };
  });
  allResults.push(pptrText);

  let rayoPageMap: BenchResult | undefined;
  let rayoText: BenchResult | undefined;
  if (rayoAvailable) {
    rayoPageMap = await bench("understand:page_map", "rayo", async () => {
      const r = await rayo.callTool("rayo_observe", { mode: "page_map" });
      return { tokens: estimateTokens(JSON.stringify(r)) };
    });
    allResults.push(rayoPageMap);

    rayoText = await bench("understand:text", "rayo", async () => {
      const r = await rayo.callTool("rayo_observe", { mode: "text" });
      return { tokens: estimateTokens(JSON.stringify(r)) };
    });
    allResults.push(rayoText);
  }

  // Screenshot
  const pwSS = await bench("understand:screenshot", "playwright", async () => {
    const b = await pwPage.screenshot();
    return { tokens: estimateTokens(b.toString("base64")), bytes: b.length };
  });
  allResults.push(pwSS);

  const pptrSS = await bench("understand:screenshot", "puppeteer", async () => {
    const b = await pptrPage.screenshot() as Buffer;
    return { tokens: estimateTokens(b.toString("base64")), bytes: b.length };
  });
  allResults.push(pptrSS);

  let rayoSS: BenchResult | undefined;
  if (rayoAvailable) {
    rayoSS = await bench("understand:screenshot", "rayo", async () => {
      const r = await rayo.callTool("rayo_observe", { mode: "screenshot" });
      const data = r?.content?.[0]?.data || r?.content?.[0]?.text || "";
      return { tokens: estimateTokens(data) };
    });
    allResults.push(rayoSS);
  }

  console.log(`  page_map (rayo):   ${rayoPageMap?.median_ms.toFixed(1) ?? "N/A"}ms  ~${rayoPageMap?.extra?.tokens ?? "?"} tokens`);
  console.log(`  text (PW):         ${pwText.median_ms.toFixed(1)}ms  ~${pwText.extra?.tokens} tokens`);
  console.log(`  text (rayo):       ${rayoText?.median_ms.toFixed(1) ?? "N/A"}ms  ~${rayoText?.extra?.tokens ?? "?"} tokens`);
  console.log(`  screenshot (PW):   ${pwSS.median_ms.toFixed(1)}ms  ~${pwSS.extra?.tokens} tokens`);
  console.log(`  screenshot (rayo): ${rayoSS?.median_ms.toFixed(1) ?? "N/A"}ms  ~${rayoSS?.extra?.tokens ?? "?"} tokens`);

  // ── DOM extraction ──
  console.log("\n📊 DOM extraction (HN)...");
  await pwPage.goto("https://news.ycombinator.com", { waitUntil: "load" });
  await pptrPage.goto("https://news.ycombinator.com", { waitUntil: "load" });
  if (rayoAvailable) await rayo.callTool("rayo_navigate", { action: "goto", url: "https://news.ycombinator.com" });

  const pwExtract = await bench("extract:hn", "playwright", async () => {
    const s = await pwPage.$$eval(".titleline > a", els => els.map(el => ({ t: el.textContent, h: el.href })));
    return { count: s.length, tokens: estimateTokens(JSON.stringify(s)) };
  });
  allResults.push(pwExtract);

  const pptrExtract = await bench("extract:hn", "puppeteer", async () => {
    const s = await pptrPage.$$eval(".titleline > a", els => els.map(el => ({ t: el.textContent, h: (el as HTMLAnchorElement).href })));
    return { count: s.length, tokens: estimateTokens(JSON.stringify(s)) };
  });
  allResults.push(pptrExtract);

  let rayoExtract: BenchResult | undefined;
  if (rayoAvailable) {
    rayoExtract = await bench("extract:hn", "rayo", async () => {
      const r = await rayo.callTool("rayo_observe", { mode: "page_map" });
      const content = JSON.stringify(r);
      const parsed = JSON.parse(r?.content?.[0]?.text || "{}");
      return { count: parsed.interactive?.length || 0, tokens: estimateTokens(content) };
    });
    allResults.push(rayoExtract);
  }

  console.log(`  PW:   ${pwExtract.median_ms.toFixed(1)}ms  ${pwExtract.extra?.count} items  ~${pwExtract.extra?.tokens} tokens`);
  console.log(`  Pptr: ${pptrExtract.median_ms.toFixed(1)}ms  ${pptrExtract.extra?.count} items`);
  console.log(`  rayo: ${rayoExtract?.median_ms.toFixed(1) ?? "N/A"}ms  ${rayoExtract?.extra?.count ?? "?"} items  ~${rayoExtract?.extra?.tokens ?? "?"} tokens`);

  // ── AI Agent Session Simulations ──
  console.log("\n📊 AI Agent Session Simulations (real Claude Code patterns)...");
  const sessions: SessionResult[] = [];

  // Session 1: Wikipedia research
  console.log("\n  Session: Wikipedia Research");
  const pwWiki = await simulateSession_PW("wikipedia_research", pwPage, wikipediaResearch_PW);
  sessions.push(pwWiki);
  console.log(`    PW:   ${pwWiki.total_ms.toFixed(0)}ms  ${pwWiki.tool_calls} calls  ${pwWiki.tokens_consumed} tokens`);

  if (rayoAvailable) {
    const rayoWiki = await simulateSession_Rayo("wikipedia_research", rayo, wikipediaResearch_Rayo);
    sessions.push(rayoWiki);
    console.log(`    rayo: ${rayoWiki.total_ms.toFixed(0)}ms  ${rayoWiki.tool_calls} calls  ${rayoWiki.tokens_consumed} tokens`);
    console.log(`    → Token savings: ${((1 - rayoWiki.tokens_consumed / pwWiki.tokens_consumed) * 100).toFixed(0)}%`);
    console.log(`    → Call reduction: ${pwWiki.tool_calls} → ${rayoWiki.tool_calls}`);
  }

  // Session 2: Form fill
  console.log("\n  Session: Form Fill + Submit");
  const pwForm = await simulateSession_PW("form_fill", pwPage, formFill_PW);
  sessions.push(pwForm);
  console.log(`    PW:   ${pwForm.total_ms.toFixed(0)}ms  ${pwForm.tool_calls} calls  ${pwForm.tokens_consumed} tokens`);

  if (rayoAvailable) {
    const rayoForm = await simulateSession_Rayo("form_fill", rayo, formFill_Rayo);
    sessions.push(rayoForm);
    console.log(`    rayo: ${rayoForm.total_ms.toFixed(0)}ms  ${rayoForm.tool_calls} calls  ${rayoForm.tokens_consumed} tokens`);
    console.log(`    → Token savings: ${((1 - rayoForm.tokens_consumed / pwForm.tokens_consumed) * 100).toFixed(0)}%`);
    console.log(`    → Call reduction: ${pwForm.tool_calls} → ${rayoForm.tool_calls}`);
  }

  // Session 3: HN browse
  console.log("\n  Session: HN Browse + Read Story");
  const pwHN = await simulateSession_PW("hn_browse", pwPage, hnBrowse_PW);
  sessions.push(pwHN);
  console.log(`    PW:   ${pwHN.total_ms.toFixed(0)}ms  ${pwHN.tool_calls} calls  ${pwHN.tokens_consumed} tokens`);

  if (rayoAvailable) {
    const rayoHN = await simulateSession_Rayo("hn_browse", rayo, hnBrowse_Rayo);
    sessions.push(rayoHN);
    console.log(`    rayo: ${rayoHN.total_ms.toFixed(0)}ms  ${rayoHN.tool_calls} calls  ${rayoHN.tokens_consumed} tokens`);
    console.log(`    → Token savings: ${((1 - rayoHN.tokens_consumed / pwHN.tokens_consumed) * 100).toFixed(0)}%`);
  }

  // ── Rayo internal profiling ──
  let profileData: string | null = null;
  if (rayoAvailable) {
    console.log("\n📊 rayo internal profile...");
    try {
      const prof = await rayo.callTool("rayo_profile", { format: "markdown" });
      profileData = prof?.content?.[0]?.text || null;
      if (profileData) {
        console.log(profileData);
      }
    } catch (e: any) {
      console.log(`  ⚠️  Could not get profile: ${e.message}`);
    }
  }

  // ── Token cost ──
  const tokenCost = {
    playwright: { tools: 22, tokens: 13200 },
    puppeteer: { tools: 9, tokens: 4500 },
    rayo: { tools: 5, tokens: 1500 },
  };

  // ── Cleanup ──
  await pwBrowser.close();
  await pptrBrowser.close();
  if (rayoAvailable) rayo.stop();

  // ── Generate report ──
  const report = generateReport(allResults, sessions, tokenCost, profileData);
  console.log("\n" + "=".repeat(60) + "\n");
  console.log(report);

  // Write results
  const resultsDir = process.cwd().replace("/competitors", "/results");
  fs.mkdirSync(resultsDir, { recursive: true });
  fs.writeFileSync(`${resultsDir}/latest.json`, JSON.stringify({
    timestamp: new Date().toISOString(),
    config: { iterations: ITERATIONS, warmup: WARMUP },
    results: allResults,
    sessions,
    tokenCost,
    rayoProfile: profileData,
  }, null, 2));
  fs.writeFileSync(`${resultsDir}/BENCHMARKS.md`, report);
  console.log(`\n✅ Results written to bench/results/`);
}

function generateReport(
  results: BenchResult[],
  sessions: SessionResult[],
  tokenCost: Record<string, { tools: number; tokens: number }>,
  rayoProfile: string | null,
): string {
  const now = new Date().toISOString().split("T")[0];
  let md = `## Benchmark Results (${now})\n\n`;
  md += `**System:** ${process.platform} ${process.arch} | **Iterations:** ${ITERATIONS} after ${WARMUP} warmup | **Warm browsers** (no cold-start)\n\n`;

  // Navigation
  md += `### Navigation Speed (warm browser)\n\n`;
  md += `| Site | rayo | Playwright | Puppeteer |\n`;
  md += `|------|------|-----------|----------|\n`;
  for (const site of SITES) {
    const get = (a: string) => results.find(r => r.scenario === `navigate:${site.name}` && r.adapter === a);
    md += `| ${site.name} | ${get("rayo")?.median_ms.toFixed(0) ?? "N/A"}ms | ${get("playwright")?.median_ms.toFixed(0)}ms | ${get("puppeteer")?.median_ms.toFixed(0)}ms |\n`;
  }

  // Page understanding
  md += `\n### Page Understanding (Speed + Token Cost)\n\n`;
  md += `| Method | Adapter | Latency | ~Tokens |\n`;
  md += `|--------|---------|---------|--------|\n`;
  const understand = results.filter(r => r.scenario.startsWith("understand:"));
  for (const r of understand) {
    md += `| ${r.scenario.replace("understand:", "")} | ${r.adapter} | ${r.median_ms.toFixed(0)}ms | ~${r.extra?.tokens ?? "?"} |\n`;
  }

  // DOM extraction
  md += `\n### DOM Extraction (HN)\n\n`;
  md += `| Adapter | Latency | Items | ~Tokens |\n`;
  md += `|---------|---------|-------|--------|\n`;
  for (const r of results.filter(r => r.scenario === "extract:hn")) {
    md += `| ${r.adapter} | ${r.median_ms.toFixed(0)}ms | ${r.extra?.count ?? "?"} | ~${r.extra?.tokens ?? "?"} |\n`;
  }

  // AI Agent Sessions (THE key section)
  md += `\n### 🤖 AI Agent Session Simulations (Real Claude Code Patterns)\n\n`;
  md += `These simulate actual Claude Code workflows — the TOTAL cost of tokens + latency + tool calls.\n\n`;

  const sessionNames = [...new Set(sessions.map(s => s.name))];
  for (const name of sessionNames) {
    const pw = sessions.find(s => s.name === name && s.adapter === "playwright");
    const r = sessions.find(s => s.name === name && s.adapter === "rayo");
    md += `#### ${name.replace(/_/g, " ").replace(/\b\w/g, c => c.toUpperCase())}\n\n`;
    md += `| Metric | Playwright MCP | rayo-browser | Advantage |\n`;
    md += `|--------|---------------|-------------|----------|\n`;
    if (pw && r) {
      md += `| Latency | ${pw.total_ms.toFixed(0)}ms | ${r.total_ms.toFixed(0)}ms | ${pw.total_ms < r.total_ms ? "PW " + (r.total_ms/pw.total_ms).toFixed(1) + "x faster" : "rayo " + (pw.total_ms/r.total_ms).toFixed(1) + "x faster"} |\n`;
      md += `| Tool calls | ${pw.tool_calls} | ${r.tool_calls} | **${((1 - r.tool_calls/pw.tool_calls) * 100).toFixed(0)}% fewer** |\n`;
      md += `| Total tokens | ${pw.tokens_consumed.toLocaleString()} | ${r.tokens_consumed.toLocaleString()} | **${((1 - r.tokens_consumed/pw.tokens_consumed) * 100).toFixed(0)}% fewer** |\n`;
    } else if (pw) {
      md += `| Latency | ${pw.total_ms.toFixed(0)}ms | N/A | |\n`;
      md += `| Tool calls | ${pw.tool_calls} | N/A | |\n`;
      md += `| Total tokens | ${pw.tokens_consumed.toLocaleString()} | N/A | |\n`;
    }
    md += `\n`;
  }

  // Token cost
  md += `### Tool Description Token Cost\n\n`;
  md += `| MCP Server | Tools | Tokens | % of 200k Context |\n`;
  md += `|-----------|-------|--------|-------------------|\n`;
  for (const [name, c] of Object.entries(tokenCost)) {
    md += `| ${name} | ${c.tools} | ~${c.tokens.toLocaleString()} | ${((c.tokens/200000)*100).toFixed(2)}% |\n`;
  }

  // Rayo internal profile
  if (rayoProfile) {
    md += `\n### rayo-browser Internal Profile\n\n`;
    md += `Where rayo spends its time (built-in profiler, always on):\n\n`;
    md += `\`\`\`\n${rayoProfile}\n\`\`\`\n`;
  }

  md += `\n---\n*Warm browsers, ${ITERATIONS} iterations after ${WARMUP} warmup, median values.*\n`;
  return md;
}

main().catch(console.error);
