import { useState } from "react";
import { RotateCcw } from "lucide-react";
import * as api from "@/lib/tauri";

interface DatabaseErrorProps {
  error: string;
  onReset: () => void;
}

function anim(delayMs: number): React.CSSProperties {
  return {
    animation: "onb-fade-up 0.5s cubic-bezier(0.22,1,0.36,1) both",
    animationDelay: `${delayMs}ms`,
  };
}

export function DatabaseError({ error, onReset }: DatabaseErrorProps) {
  const [resetting, setResetting] = useState(false);
  const [resetError, setResetError] = useState<string | null>(null);

  const handleReset = async () => {
    setResetting(true);
    setResetError(null);
    try {
      await api.resetDatabase();
      onReset();
    } catch (e) {
      setResetError(e instanceof Error ? e.message : String(e));
      setResetting(false);
    }
  };

  return (
    <div
      className="dark relative flex h-screen w-screen items-center justify-center overflow-hidden select-none"
      style={{
        background:
          "radial-gradient(ellipse 80% 60% at 50% 0%, #1C1210 0%, #0C0A09 55%, #080706 100%)",
        fontFamily: '"DM Sans", system-ui, sans-serif',
        color: "#FAF9F8",
      }}
    >
      {/* Dot grid */}
      <div
        className="pointer-events-none absolute inset-0"
        style={{
          opacity: 0.025,
          backgroundImage:
            "radial-gradient(circle, rgba(232,98,62,0.6) 1px, transparent 1px)",
          backgroundSize: "28px 28px",
        }}
      />

      {/* Top accent line */}
      <div
        className="absolute top-0 inset-x-0 h-[2px] z-20"
        style={{
          background:
            "linear-gradient(90deg, transparent 5%, #E8623E 35%, #F59E6B 65%, transparent 95%)",
        }}
      />

      {/* Ambient glow — warm amber, muted */}
      <div
        className="pointer-events-none absolute -top-40 left-1/2 -translate-x-1/2 w-[600px] h-[400px]"
        style={{
          background: "radial-gradient(ellipse, #E8623E, transparent 70%)",
          opacity: 0.08,
          animation: "onb-glow-pulse 6s ease-in-out infinite",
        }}
      />

      <div className="relative z-10 flex max-w-md flex-col items-center px-8 text-center">
        {/* Broken-DB icon */}
        <div style={anim(0)} className="mb-8">
          <div
            className="relative flex h-20 w-20 items-center justify-center rounded-2xl"
            style={{
              background: "rgba(232,98,62,0.08)",
              border: "1px solid rgba(232,98,62,0.15)",
              boxShadow: "0 0 40px -10px rgba(232,98,62,0.2)",
            }}
          >
            <svg
              width="36"
              height="36"
              viewBox="0 0 24 24"
              fill="none"
              stroke="#E8623E"
              strokeWidth="1.5"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              {/* Database cylinder */}
              <ellipse cx="12" cy="5" rx="8" ry="3" />
              <path d="M4 5v6c0 1.66 3.58 3 8 3s8-1.34 8-3V5" />
              <path d="M4 11v6c0 1.66 3.58 3 8 3s8-1.34 8-3v-6" />
              {/* Crack / break line */}
              <path
                d="M9 8l2 3-2 3 2 3"
                stroke="#E8623E"
                strokeWidth="1.5"
                opacity="0.7"
              />
            </svg>
          </div>
        </div>

        {/* Heading */}
        <h1
          style={{
            fontFamily: '"Syne", sans-serif',
            fontSize: "1.5rem",
            fontWeight: 700,
            letterSpacing: "0.04em",
            ...anim(80),
          }}
        >
          Database Error
        </h1>

        {/* Description */}
        <p
          className="mt-3 leading-relaxed"
          style={{
            color: "rgba(250,249,248,0.55)",
            fontSize: "0.875rem",
            ...anim(160),
          }}
        >
          The database could not be initialized. This may be caused by a
          corrupted database file or a system issue. You can reset the
          database to start fresh.
        </p>

        {/* Technical details */}
        <details
          className="mt-6 w-full text-left"
          style={anim(240)}
        >
          <summary
            className="cursor-pointer text-xs transition-colors"
            style={{ color: "rgba(250,249,248,0.35)" }}
            onMouseEnter={(e) =>
              (e.currentTarget.style.color = "rgba(250,249,248,0.65)")
            }
            onMouseLeave={(e) =>
              (e.currentTarget.style.color = "rgba(250,249,248,0.35)")
            }
          >
            Technical details
          </summary>
          <pre
            className="mt-2 max-h-32 overflow-auto rounded-lg p-3 text-xs whitespace-pre-wrap break-words"
            style={{
              background: "rgba(255,255,255,0.04)",
              border: "1px solid rgba(255,255,255,0.06)",
              color: "rgba(250,249,248,0.45)",
            }}
          >
            {error}
          </pre>
        </details>

        {/* Reset error feedback */}
        {resetError && (
          <p
            className="mt-4 rounded-lg px-3 py-2 text-xs"
            style={{
              background: "rgba(232,98,62,0.1)",
              border: "1px solid rgba(232,98,62,0.2)",
              color: "#F59E6B",
            }}
          >
            Reset failed: {resetError}
          </p>
        )}

        {/* Reset button */}
        <button
          onClick={handleReset}
          disabled={resetting}
          className="mt-8 inline-flex items-center gap-2.5 rounded-xl px-6 py-3 text-sm font-medium transition-all disabled:opacity-50"
          style={{
            background:
              "linear-gradient(135deg, #E8623E 0%, #D4522E 100%)",
            color: "#fff",
            boxShadow:
              "0 0 20px -4px rgba(232,98,62,0.4), 0 2px 8px rgba(0,0,0,0.3)",
            ...anim(320),
          }}
          onMouseEnter={(e) => {
            if (!resetting) {
              e.currentTarget.style.boxShadow =
                "0 0 30px -4px rgba(232,98,62,0.6), 0 4px 12px rgba(0,0,0,0.4)";
              e.currentTarget.style.transform = "translateY(-1px)";
            }
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.boxShadow =
              "0 0 20px -4px rgba(232,98,62,0.4), 0 2px 8px rgba(0,0,0,0.3)";
            e.currentTarget.style.transform = "translateY(0)";
          }}
        >
          <RotateCcw
            className={`h-4 w-4 ${resetting ? "animate-spin" : ""}`}
          />
          {resetting ? "Resetting…" : "Reset Database"}
        </button>

        {/* Reassurance note */}
        <p
          className="mt-4"
          style={{
            color: "rgba(250,249,248,0.25)",
            fontSize: "0.7rem",
            ...anim(400),
          }}
        >
          This will remove all local data and start the setup wizard.
        </p>
      </div>
    </div>
  );
}
