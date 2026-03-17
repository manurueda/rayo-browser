"use client";

import { useEffect, useState } from "react";
import { fetchResults, fetchSuites, runAllSuites, SuiteResult, SuiteSummary } from "@/lib/api";
import { DashboardStats } from "@/components/dashboard-stats";
import { SuiteCard } from "@/components/suite-card";

export default function DashboardPage() {
  const [suites, setSuites] = useState<SuiteSummary[]>([]);
  const [results, setResults] = useState<SuiteResult[]>([]);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetchSuites().then(setSuites).catch(() => setError("Failed to connect to rayo-test server"));
    fetchResults().then(setResults).catch(() => {});
  }, []);

  const handleRunAll = async () => {
    setRunning(true);
    setError(null);
    try {
      const newResults = await runAllSuites();
      setResults((prev) => [...prev, ...newResults]);
    } catch (e) {
      setError("Failed to run tests");
    } finally {
      setRunning(false);
    }
  };

  const latestResults = results.slice(-10).reverse();

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Dashboard</h1>
          <p className="text-[var(--muted)] text-sm mt-1">
            {suites.length} test suite{suites.length !== 1 ? "s" : ""} available
          </p>
        </div>
        <button
          onClick={handleRunAll}
          disabled={running}
          className="px-4 py-2 rounded-lg bg-[var(--accent)] text-white font-medium text-sm hover:opacity-90 transition-opacity disabled:opacity-50"
        >
          {running ? "Running..." : "Run All Tests"}
        </button>
      </div>

      {error && (
        <div className="rounded-lg border border-red-900 bg-red-950/20 p-4 text-red-400 text-sm">
          {error}
          <span className="block text-xs mt-1 opacity-70">
            Make sure rayo-test server is running: rayo-test ui
          </span>
        </div>
      )}

      <DashboardStats results={latestResults} />

      <div>
        <h2 className="text-lg font-semibold mb-3">Recent Runs</h2>
        {latestResults.length === 0 ? (
          <div className="rounded-lg border border-[var(--card-border)] bg-[var(--card)] p-8 text-center text-[var(--muted)]">
            <p>No test results yet.</p>
            <p className="text-sm mt-2">Run your test suites to see results here.</p>
          </div>
        ) : (
          <div className="grid gap-4 md:grid-cols-2">
            {latestResults.map((result, i) => (
              <SuiteCard key={i} result={result} />
            ))}
          </div>
        )}
      </div>

      <div>
        <h2 className="text-lg font-semibold mb-3">Available Suites</h2>
        <div className="grid gap-3">
          {suites.map((suite) => (
            <div
              key={suite.name}
              className="rounded-lg border border-[var(--card-border)] bg-[var(--card)] p-4 flex items-center justify-between"
            >
              <div>
                <div className="font-medium">{suite.name}</div>
                <div className="text-xs text-[var(--muted)]">
                  {suite.steps} steps
                  {suite.has_setup ? " · setup" : ""}
                  {suite.has_teardown ? " · teardown" : ""}
                </div>
              </div>
              <div className="text-xs text-[var(--muted)] font-mono">{suite.path}</div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
