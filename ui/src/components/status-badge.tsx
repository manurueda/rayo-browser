export function StatusBadge({ pass }: { pass: boolean }) {
  return (
    <span
      className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${
        pass
          ? "bg-green-900/50 text-green-400 border border-green-800"
          : "bg-red-900/50 text-red-400 border border-red-800"
      }`}
    >
      {pass ? "PASS" : "FAIL"}
    </span>
  );
}
