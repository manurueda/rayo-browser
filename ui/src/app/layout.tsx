import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "rayo-ui",
  description: "AI-native E2E visual testing dashboard",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" className="dark">
      <body>
        <div className="min-h-screen">
          <nav className="border-b border-[var(--card-border)] px-6 py-3">
            <div className="flex items-center justify-between max-w-7xl mx-auto">
              <div className="flex items-center gap-3">
                <span className="text-xl font-bold">
                  ⚡ rayo<span className="text-[var(--accent)]">-ui</span>
                </span>
              </div>
              <div className="flex items-center gap-4 text-sm text-[var(--muted)]">
                <a href="/" className="hover:text-white transition-colors">
                  Dashboard
                </a>
                <a href="/suites" className="hover:text-white transition-colors">
                  Suites
                </a>
                <a href="/live" className="hover:text-white transition-colors">
                  Live
                </a>
              </div>
            </div>
          </nav>
          <main className="max-w-7xl mx-auto px-6 py-6">{children}</main>
        </div>
      </body>
    </html>
  );
}
