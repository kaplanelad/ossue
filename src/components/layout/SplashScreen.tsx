import { Inbox, GitPullRequest, CircleDot, MessageSquare } from "lucide-react";
import type { LucideIcon } from "lucide-react";

const nodes: { icon: LucideIcon; label: string }[] = [
  { icon: Inbox, label: "Inbox" },
  { icon: GitPullRequest, label: "PRs" },
  { icon: CircleDot, label: "Issues" },
  { icon: MessageSquare, label: "Chat" },
];

export function SplashScreen() {
  return (
    <>
      <style>{`
        @keyframes node-activate {
          0%, 100% {
            opacity: 0.15;
            transform: scale(0.92);
            box-shadow: 0 0 0 0 transparent;
          }
          15%, 45% {
            opacity: 1;
            transform: scale(1);
            box-shadow:
              0 0 20px -4px oklch(0.65 0.15 250 / 0.4),
              0 0 40px -8px oklch(0.65 0.15 250 / 0.15);
          }
          60% {
            opacity: 0.85;
            transform: scale(0.98);
            box-shadow:
              0 0 12px -4px oklch(0.65 0.15 250 / 0.2),
              0 0 24px -8px oklch(0.65 0.15 250 / 0.08);
          }
          75% {
            opacity: 0.4;
            transform: scale(0.95);
            box-shadow: 0 0 0 0 transparent;
          }
        }

        @keyframes line-fill {
          0%, 100% { transform: scaleX(0); opacity: 0; }
          12%, 50% { transform: scaleX(1); opacity: 1; }
          70% { transform: scaleX(1); opacity: 0; }
        }

        @keyframes label-show {
          0%, 100% { opacity: 0; transform: translateY(2px); }
          15%, 45% { opacity: 0.7; transform: translateY(0); }
          70% { opacity: 0; transform: translateY(2px); }
        }

        @keyframes title-enter {
          from { opacity: 0; letter-spacing: 0.3em; }
          to { opacity: 1; letter-spacing: 0.12em; }
        }

        @keyframes subtitle-enter {
          from { opacity: 0; }
          to { opacity: 0.5; }
        }

        @keyframes bg-pulse {
          0%, 100% { opacity: 0.3; }
          50% { opacity: 0.6; }
        }

        .splash-node {
          animation: node-activate 3.2s cubic-bezier(0.4, 0, 0.2, 1) infinite;
          opacity: 0.15;
        }

        .splash-line {
          animation: line-fill 3.2s cubic-bezier(0.4, 0, 0.2, 1) infinite;
          transform-origin: left center;
          transform: scaleX(0);
        }

        .splash-label {
          animation: label-show 3.2s cubic-bezier(0.4, 0, 0.2, 1) infinite;
          opacity: 0;
        }
      `}</style>

      <div className="fixed inset-0 flex items-center justify-center bg-background overflow-hidden">
        {/* Ambient background glow */}
        <div
          className="pointer-events-none absolute inset-0"
          style={{
            background:
              "radial-gradient(ellipse 50% 40% at 50% 45%, oklch(0.65 0.15 250 / 0.06), transparent)",
            animation: "bg-pulse 3.2s ease-in-out infinite",
          }}
        />

        <div className="relative flex flex-col items-center gap-12">
          {/* Pipeline row */}
          <div className="flex items-center">
            {nodes.map((node, i) => (
              <div key={node.label} className="flex items-center">
                {/* Node */}
                <div className="flex flex-col items-center gap-3">
                  <div
                    className="splash-node flex h-14 w-14 items-center justify-center rounded-2xl border border-border/60 bg-card/80"
                    style={{ animationDelay: `${i * 0.5}s` }}
                  >
                    <node.icon className="h-6 w-6 text-foreground" strokeWidth={1.5} />
                  </div>
                  <span
                    className="splash-label text-[10px] font-medium uppercase tracking-widest text-muted-foreground"
                    style={{ animationDelay: `${i * 0.5}s` }}
                  >
                    {node.label}
                  </span>
                </div>

                {/* Connector line */}
                {i < nodes.length - 1 && (
                  <div className="relative mx-3 mb-7 h-px w-10">
                    <div className="absolute inset-0 bg-border/30" />
                    <div
                      className="splash-line absolute inset-0 bg-foreground/40"
                      style={{ animationDelay: `${i * 0.5 + 0.25}s` }}
                    />
                  </div>
                )}
              </div>
            ))}
          </div>

          {/* Title block */}
          <div className="flex flex-col items-center gap-2">
            <div className="flex items-center gap-2.5" style={{ animation: "title-enter 0.8s cubic-bezier(0.16, 1, 0.3, 1) 0.2s both" }}>
              <img src="/app-icon.png" alt="" className="h-7 w-7" />
              <h1 className="text-lg font-semibold tracking-[0.12em] text-foreground">
                OSSUE
              </h1>
            </div>
            <p
              className="text-xs text-muted-foreground"
              style={{
                animation: "subtitle-enter 0.6s ease 0.6s both",
              }}
            >
              Loading workspace…
            </p>
          </div>
        </div>
      </div>
    </>
  );
}
