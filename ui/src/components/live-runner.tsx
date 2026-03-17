"use client";

import { useEffect, useRef, useState } from "react";
import { connectLive, TestEvent, StepResult } from "@/lib/api";

export function LiveRunner() {
  const [events, setEvents] = useState<TestEvent[]>([]);
  const [connected, setConnected] = useState(false);
  const [currentSuite, setCurrentSuite] = useState<string | null>(null);
  const [completedSteps, setCompletedSteps] = useState<StepResult[]>([]);
  const [progress, setProgress] = useState({ current: 0, total: 0 });
  const wsRef = useRef<WebSocket | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const ws = connectLive((event) => {
      setEvents((prev) => [...prev.slice(-100), event]);

      if (event.type === "SuiteStarted") {
        setCurrentSuite(event.name || null);
        setCompletedSteps([]);
        setProgress({ current: 0, total: event.total_steps || 0 });
      } else if (event.type === "StepCompleted" && event.result) {
        setCompletedSteps((prev) => [...prev, event.result as StepResult]);
        setProgress((prev) => ({ ...prev, current: prev.current + 1 }));
      } else if (event.type === "SuiteCompleted") {
        setCurrentSuite(null);
      }
    });

    ws.onopen = () => setConnected(true);
    ws.onclose = () => setConnected(false);
    wsRef.current = ws;

    return () => ws.close();
  }, []);

  useEffect(() => {
    scrollRef.current?.scrollTo({
      top: scrollRef.current.scrollHeight,
      behavior: "smooth",
    });
  }, [completedSteps]);

  const progressPercent =
    progress.total > 0 ? (progress.current / progress.total) * 100 : 0;

  return (
    <div className="rounded-lg border border-[var(--card-border)] bg-[var(--card)] p-5">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-lg font-semibold">Live Runner</h2>
        <div className="flex items-center gap-2">
          <div
            className={`w-2 h-2 rounded-full ${
              connected ? "bg-green-400" : "bg-red-400"
            }`}
          />
          <span className="text-xs text-[var(--muted)]">
            {connected ? "Connected" : "Disconnected"}
          </span>
        </div>
      </div>

      {currentSuite && (
        <div className="mb-4">
          <div className="flex items-center justify-between text-sm mb-2">
            <span className="font-medium">{currentSuite}</span>
            <span className="text-[var(--muted)]">
              {progress.current}/{progress.total}
            </span>
          </div>
          <div className="h-2 bg-[var(--background)] rounded-full overflow-hidden">
            <div
              className="h-full bg-[var(--accent)] rounded-full transition-all duration-300"
              style={{ width: `${progressPercent}%` }}
            />
          </div>
        </div>
      )}

      <div
        ref={scrollRef}
        className="max-h-[400px] overflow-y-auto space-y-1.5"
      >
        {completedSteps.length === 0 && !currentSuite && (
          <div className="text-[var(--muted)] text-sm text-center py-8">
            Waiting for test execution...
            <br />
            <span className="text-xs">
              Run tests via CLI or POST /api/run to see live updates
            </span>
          </div>
        )}
        {completedSteps.map((step, i) => (
          <div
            key={i}
            className="flex items-center gap-2 text-sm py-1.5 px-2 rounded bg-[var(--background)]"
          >
            <span
              className={step.pass ? "text-green-400" : "text-red-400"}
            >
              {step.pass ? "✓" : "✗"}
            </span>
            <span className="flex-1">{step.name}</span>
            <span className="text-xs text-[var(--muted)]">
              {step.duration_ms}ms
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
