"use client";

import { SuiteResult } from "@/lib/api";

export function DashboardStats({ results }: { results: SuiteResult[] }) {
  const totalSuites = results.length;
  const passedSuites = results.filter((r) => r.pass).length;
  const failedSuites = totalSuites - passedSuites;
  const passRate = totalSuites > 0 ? (passedSuites / totalSuites) * 100 : 0;
  const totalDuration = results.reduce((sum, r) => sum + r.duration_ms, 0);
  const totalSteps = results.reduce((sum, r) => sum + r.total_steps, 0);

  const stats = [
    {
      label: "Pass Rate",
      value: `${passRate.toFixed(0)}%`,
      color: passRate >= 80 ? "text-green-400" : passRate >= 50 ? "text-yellow-400" : "text-red-400",
    },
    {
      label: "Suites",
      value: `${passedSuites}/${totalSuites}`,
      color: failedSuites === 0 ? "text-green-400" : "text-red-400",
    },
    {
      label: "Steps",
      value: totalSteps.toString(),
      color: "text-[var(--accent)]",
    },
    {
      label: "Duration",
      value: totalDuration > 1000 ? `${(totalDuration / 1000).toFixed(1)}s` : `${totalDuration}ms`,
      color: "text-[var(--muted)]",
    },
  ];

  return (
    <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
      {stats.map((stat) => (
        <div
          key={stat.label}
          className="rounded-lg border border-[var(--card-border)] bg-[var(--card)] p-4"
        >
          <div className="text-xs text-[var(--muted)] uppercase tracking-wider mb-1">
            {stat.label}
          </div>
          <div className={`text-2xl font-bold ${stat.color}`}>{stat.value}</div>
        </div>
      ))}
    </div>
  );
}
