"use client";

import { SuiteResult } from "@/lib/api";
import { StatusBadge } from "./status-badge";

export function SuiteCard({
  result,
  onClick,
}: {
  result: SuiteResult;
  onClick?: () => void;
}) {
  return (
    <div
      onClick={onClick}
      className={`rounded-lg border p-5 cursor-pointer transition-colors hover:border-[var(--accent)] ${
        result.pass
          ? "border-[var(--card-border)] bg-[var(--card)]"
          : "border-red-900/50 bg-[var(--card)]"
      }`}
    >
      <div className="flex items-center justify-between mb-3">
        <h3 className="font-semibold text-lg">{result.name}</h3>
        <StatusBadge pass={result.pass} />
      </div>

      <div className="flex items-center gap-4 text-sm text-[var(--muted)]">
        <span>
          <span className="text-green-400 font-medium">
            {result.passed_steps}
          </span>
          /{result.total_steps} steps
        </span>
        {result.failed_steps > 0 && (
          <span className="text-red-400">
            {result.failed_steps} failed
          </span>
        )}
        <span>{result.duration_ms}ms</span>
      </div>

      {/* Step progress bar */}
      <div className="flex gap-0.5 mt-3 h-1.5 rounded-full overflow-hidden">
        {result.steps.map((step, i) => (
          <div
            key={i}
            className={`flex-1 ${step.pass ? "bg-green-500" : "bg-red-500"}`}
          />
        ))}
      </div>
    </div>
  );
}
