"use client";

import { StepResult } from "@/lib/api";

export function StepCard({ step, index }: { step: StepResult; index: number }) {
  return (
    <div
      className={`rounded-lg border p-4 ${
        step.pass
          ? "border-[var(--card-border)] bg-[var(--card)]"
          : "border-red-900 bg-red-950/20"
      }`}
    >
      <div className="flex items-center gap-3">
        <span
          className={`text-lg ${step.pass ? "text-green-400" : "text-red-400"}`}
        >
          {step.pass ? "✓" : "✗"}
        </span>
        <span className="font-semibold flex-1">{step.name}</span>
        <span className="text-xs text-[var(--muted)] bg-[var(--background)] px-2 py-1 rounded">
          {step.action}
        </span>
        <span className="text-xs text-[var(--muted)]">{step.duration_ms}ms</span>
      </div>

      {step.error && (
        <div className="mt-2 text-sm text-red-400 bg-red-950/30 rounded p-2 font-mono">
          {step.error}
        </div>
      )}

      {step.assertions.length > 0 && (
        <div className="mt-3 space-y-1.5 pl-7">
          {step.assertions.map((a, i) => (
            <div key={i} className="flex items-center gap-2 text-sm">
              <span className={a.pass ? "text-green-400" : "text-red-400"}>
                {a.pass ? "✓" : "✗"}
              </span>
              <span className="text-[var(--muted)]">{a.assertion_type}</span>
              {a.message && (
                <span className="text-xs text-[var(--muted)] opacity-70">
                  — {a.message}
                </span>
              )}
              {a.new_baseline && (
                <span className="text-xs text-yellow-400 bg-yellow-900/30 px-1.5 py-0.5 rounded">
                  new baseline
                </span>
              )}
            </div>
          ))}
        </div>
      )}

      {step.assertions.some((a) => a.diff_report) && (
        <div className="mt-3 pl-7">
          {step.assertions
            .filter((a) => a.diff_report)
            .map((a, i) => (
              <div
                key={i}
                className="text-xs bg-[var(--background)] rounded p-3 font-mono space-y-1"
              >
                <div>
                  Diff: {((a.diff_report!.diff_ratio) * 100).toFixed(2)}% pixels
                  changed
                </div>
                <div>
                  Perceptual score: {a.diff_report!.perceptual_score.toFixed(4)}
                </div>
                <div>
                  Dimensions: {a.diff_report!.dimensions[0]}x
                  {a.diff_report!.dimensions[1]}
                </div>
                <div>
                  Timing: {(a.diff_report!.timing.total_us / 1000).toFixed(1)}ms
                </div>
                {a.diff_report!.changed_regions.length > 0 && (
                  <div>
                    Regions:{" "}
                    {a.diff_report!.changed_regions.map((r, j) => (
                      <span key={j} className="text-yellow-400">
                        [{r.x},{r.y} {r.width}x{r.height}]{" "}
                      </span>
                    ))}
                  </div>
                )}
              </div>
            ))}
        </div>
      )}
    </div>
  );
}
