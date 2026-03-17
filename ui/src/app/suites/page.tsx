"use client";

import { useEffect, useState } from "react";
import { fetchResults, runSuite, SuiteResult } from "@/lib/api";
import { SuiteCard } from "@/components/suite-card";
import { StepCard } from "@/components/step-card";

export default function SuitesPage() {
  const [results, setResults] = useState<SuiteResult[]>([]);
  const [selected, setSelected] = useState<SuiteResult | null>(null);
  const [running, setRunning] = useState<string | null>(null);

  useEffect(() => {
    fetchResults().then(setResults).catch(() => {});
  }, []);

  const handleRun = async (name: string) => {
    setRunning(name);
    try {
      const result = await runSuite(name);
      setResults((prev) => [...prev, result]);
      setSelected(result);
    } catch {
      // handled by UI
    } finally {
      setRunning(null);
    }
  };

  const uniqueSuites = results.reduce(
    (acc, r) => {
      acc[r.name] = r; // last result per suite name
      return acc;
    },
    {} as Record<string, SuiteResult>
  );

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">Test Suites</h1>

      {selected ? (
        <div className="space-y-4">
          <button
            onClick={() => setSelected(null)}
            className="text-sm text-[var(--accent)] hover:underline"
          >
            ← Back to suites
          </button>

          <div className="flex items-center justify-between">
            <h2 className="text-xl font-semibold">{selected.name}</h2>
            <div className="flex items-center gap-3">
              <span
                className={`text-sm font-medium ${
                  selected.pass ? "text-green-400" : "text-red-400"
                }`}
              >
                {selected.pass ? "PASSED" : "FAILED"}
              </span>
              <span className="text-sm text-[var(--muted)]">
                {selected.passed_steps}/{selected.total_steps} steps ·{" "}
                {selected.duration_ms}ms
              </span>
              <button
                onClick={() => handleRun(selected.name)}
                disabled={running === selected.name}
                className="px-3 py-1.5 rounded bg-[var(--accent)] text-white text-sm hover:opacity-90 disabled:opacity-50"
              >
                {running === selected.name ? "Running..." : "Re-run"}
              </button>
            </div>
          </div>

          <div className="space-y-3">
            {selected.steps.map((step, i) => (
              <StepCard key={i} step={step} index={i} />
            ))}
          </div>
        </div>
      ) : (
        <div className="grid gap-4 md:grid-cols-2">
          {Object.values(uniqueSuites).length === 0 ? (
            <div className="col-span-2 rounded-lg border border-[var(--card-border)] bg-[var(--card)] p-8 text-center text-[var(--muted)]">
              No results yet. Run tests from the Dashboard.
            </div>
          ) : (
            Object.values(uniqueSuites).map((result) => (
              <SuiteCard
                key={result.name}
                result={result}
                onClick={() => setSelected(result)}
              />
            ))
          )}
        </div>
      )}
    </div>
  );
}
