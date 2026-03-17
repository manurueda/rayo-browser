const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://localhost:4040";

export interface SuiteSummary {
  name: string;
  path: string;
  steps: number;
  has_setup: boolean;
  has_teardown: boolean;
}

export interface StepResult {
  name: string;
  pass: boolean;
  duration_ms: number;
  action: string;
  error?: string;
  assertions: AssertionResult[];
  page_map?: unknown;
}

export interface AssertionResult {
  assertion_type: string;
  pass: boolean;
  message?: string;
  diff_report?: DiffReport;
  new_baseline: boolean;
}

export interface DiffReport {
  pass: boolean;
  diff_ratio: number;
  diff_pixel_count: number;
  total_pixel_count: number;
  perceptual_score: number;
  changed_regions: ChangedRegion[];
  has_diff_image: boolean;
  dimensions: [number, number];
  blank_detected: boolean;
  new_baseline: boolean;
  timing: DiffTiming;
}

export interface ChangedRegion {
  x: number;
  y: number;
  width: number;
  height: number;
  diff_ratio: number;
}

export interface DiffTiming {
  decode_us: number;
  pixel_us: number;
  perceptual_us: number;
  cluster_us: number;
  overlay_us: number;
  total_us: number;
}

export interface SuiteResult {
  name: string;
  pass: boolean;
  total_steps: number;
  passed_steps: number;
  failed_steps: number;
  duration_ms: number;
  steps: StepResult[];
  started_at: string;
}

export interface TestEvent {
  type: "SuiteStarted" | "StepStarted" | "StepCompleted" | "SuiteCompleted";
  name?: string;
  total_steps?: number;
  index?: number;
  result?: StepResult | SuiteResult;
}

export async function fetchSuites(): Promise<SuiteSummary[]> {
  const res = await fetch(`${API_BASE}/api/suites`);
  const data = await res.json();
  return data.suites;
}

export async function fetchResults(): Promise<SuiteResult[]> {
  const res = await fetch(`${API_BASE}/api/results`);
  const data = await res.json();
  return data.results;
}

export async function runAllSuites(): Promise<SuiteResult[]> {
  const res = await fetch(`${API_BASE}/api/run`, { method: "POST" });
  const data = await res.json();
  return data.results;
}

export async function runSuite(name: string): Promise<SuiteResult> {
  const res = await fetch(`${API_BASE}/api/run/${encodeURIComponent(name)}`, {
    method: "POST",
  });
  const data = await res.json();
  return data.result;
}

export function connectLive(onEvent: (event: TestEvent) => void): WebSocket {
  const wsUrl = API_BASE.replace(/^http/, "ws") + "/ws/live";
  const ws = new WebSocket(wsUrl);
  ws.onmessage = (e) => {
    try {
      const event = JSON.parse(e.data) as TestEvent;
      onEvent(event);
    } catch {
      // ignore parse errors
    }
  };
  return ws;
}
