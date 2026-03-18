"use client";

import { LiveRunner } from "@/components/live-runner";
import { runAllSuites } from "@/lib/api";
import { useState } from "react";

export default function LivePage() {
  const [running, setRunning] = useState(false);

  const handleRun = async () => {
    setRunning(true);
    try {
      await runAllSuites();
    } catch {
      // errors shown in live runner
    } finally {
      setRunning(false);
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">Live Runner</h1>
        <button
          onClick={handleRun}
          disabled={running}
          className="px-4 py-2 rounded-lg bg-[var(--accent)] text-white font-medium text-sm hover:opacity-90 transition-opacity disabled:opacity-50"
        >
          {running ? "Running..." : "Run All Tests"}
        </button>
      </div>

      <LiveRunner />

      <div className="text-xs text-[var(--muted)]">
        <p>
          The live runner connects via WebSocket to the rayo-ui server and shows
          real-time step execution. Start a test run from here or via CLI.
        </p>
      </div>
    </div>
  );
}
