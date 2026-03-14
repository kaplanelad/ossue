import { useState, useEffect, Fragment } from "react";
import { errorMessage } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { RepoPicker } from "./RepoPicker";
import { GitHubOAuthFlow } from "@/components/shared/GitHubOAuthFlow";
import { useAppStore } from "@/stores/appStore";
import * as api from "@/lib/tauri";
import type { Connector } from "@/types";
import {
  Github,
  Loader2,
  ArrowRight,
  ExternalLink,
  ShieldCheck,
  GitlabIcon,
  Sparkles,
  Zap,
  Terminal,
  Monitor,
  Eye,
  EyeOff,
} from "lucide-react";

type OnboardingStep = "welcome" | "connect" | "repos" | "ai" | "done";
type AiMode = "api" | "claude_cli" | "cursor_cli";

const STEPS: { key: OnboardingStep; label: string }[] = [
  { key: "connect", label: "Connect" },
  { key: "repos", label: "Repos" },
  { key: "ai", label: "AI" },
  { key: "done", label: "Done" },
];

const AI_PROVIDERS: {
  mode: AiMode;
  title: string;
  description: string;
  speed: string;
  reviewQuality: string;
  codebaseAccess: string;
  bestFor: string;
  requires: string;
  icon: typeof Zap;
}[] = [
  {
    mode: "claude_cli",
    title: "Claude Code CLI",
    description: "Deep analysis with full codebase",
    speed: "30s-2min",
    reviewQuality: "Excellent — full repo",
    codebaseAccess: "Full project files",
    bestFor: "Thorough code review",
    requires: "claude CLI installed",
    icon: Terminal,
  },
  {
    mode: "cursor_cli",
    title: "Cursor CLI",
    description: "Deep analysis via Cursor",
    speed: "30s-2min",
    reviewQuality: "Excellent — full repo",
    codebaseAccess: "Full project files",
    bestFor: "Cursor users",
    requires: "cursor CLI installed",
    icon: Monitor,
  },
  {
    mode: "api",
    title: "API (Anthropic/OpenAI)",
    description: "Fast cloud-based analysis",
    speed: "2-5 seconds",
    reviewQuality: "Good for simple PRs",
    codebaseAccess: "PR diff + comments only",
    bestFor: "Quick triage & summaries",
    requires: "API key",
    icon: Zap,
  },
];

// Staggered fade-up helper
function anim(delayMs: number): React.CSSProperties {
  return {
    animation: "onb-fade-up 0.5s cubic-bezier(0.22,1,0.36,1) both",
    animationDelay: `${delayMs}ms`,
  };
}

export function Welcome() {
  const [step, setStep] = useState<OnboardingStep>("welcome");
  const [githubToken, setGithubToken] = useState("");
  const [gitlabToken, setGitlabToken] = useState("");
  const [gitlabBaseUrl, setGitlabBaseUrl] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [savingProvider, setSavingProvider] = useState<
    "github" | "gitlab" | null
  >(null);
  const [error, setError] = useState<string | null>(null);
  const [connectProvider, setConnectProvider] = useState<"github" | "gitlab">(
    "github",
  );
  const [connectors, setConnectors] = useState<Connector[]>([]);
  const { setCurrentPage, setProjects, setOnboardingJustCompleted } = useAppStore();

  // AI settings state
  const [aiMode, setAiMode] = useState<AiMode>("claude_cli");
  const [aiApiKey, setAiApiKey] = useState("");
  const [aiModel, setAiModel] = useState("");
  const [aiCustomInstructions, setAiCustomInstructions] = useState("");
  const [showApiKey, setShowApiKey] = useState(false);

  // Entrance animation
  const [mounted, setMounted] = useState(false);
  useEffect(() => {
    requestAnimationFrame(() => setMounted(true));
  }, []);

  // Detect onboarding progress on mount (for resume after app restart)
  useEffect(() => {
    async function detectProgress() {
      try {
        const existingConnectors = await api.listConnectors();
        if (existingConnectors.length === 0) return;

        setConnectors(existingConnectors);

        const projects = await api.listProjects();
        if (projects.length === 0) {
          setStep("repos");
          return;
        }

        // Have connectors + projects, resume at AI step
        setStep("ai");
      } catch {
        // Start from the beginning on error
      }
    }
    detectProgress();
  }, []);

  /* ── Handlers ─────────────────────────────────────────── */

  const handleConnectGithub = async () => {
    if (!githubToken.trim()) return;
    setSavingProvider("github");
    setError(null);
    try {
      const connector = await api.addConnector({
        name: "GitHub",
        platform: "github",
        token: githubToken,
      });
      setConnectors((prev) => [...prev, connector]);
      setStep("repos");
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSavingProvider(null);
    }
  };

  const handleOAuthSuccess = async (token: string) => {
    setSavingProvider("github");
    setError(null);
    try {
      const connector = await api.addConnector({
        name: "GitHub",
        platform: "github",
        token: token,
      });
      setConnectors((prev) => [...prev, connector]);
      setStep("repos");
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSavingProvider(null);
    }
  };

  const handleConnectGitlab = async () => {
    if (!gitlabToken.trim()) return;
    setSavingProvider("gitlab");
    setError(null);
    try {
      const connector = await api.addConnector({
        name: "GitLab",
        platform: "gitlab",
        token: gitlabToken,
        base_url: gitlabBaseUrl.trim() || undefined,
      });
      setConnectors((prev) => [...prev, connector]);
      setStep("repos");
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSavingProvider(null);
    }
  };

  const handleSaveAiSettings = async () => {
    setIsLoading(true);
    setError(null);
    try {
      await api.updateSetting("ai_mode", aiMode);
      if (aiMode === "api") {
        await api.updateSetting("ai_provider", "anthropic");
        if (aiApiKey.trim()) {
          await api.updateSetting("ai_api_key", aiApiKey.trim());
        }
      } else {
        await api.updateSetting("ai_provider", "cli");
      }
      if (aiModel) {
        await api.updateSetting("ai_model", aiModel);
      }
      if (aiCustomInstructions.trim()) {
        await api.updateSetting(
          "ai_custom_instructions",
          aiCustomInstructions.trim(),
        );
      }
      setStep("done");
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setIsLoading(false);
    }
  };

  const handleFinish = async () => {
    const projects = await api.listProjects();
    setProjects(projects);
    setOnboardingJustCompleted(true);
    setCurrentPage("main");
  };

  /* ── Derived ──────────────────────────────────────────── */

  const stepIndex = STEPS.findIndex((s) => s.key === step);

  // AI save validation: API mode requires an API key
  const canSaveAi =
    aiMode !== "api" || aiApiKey.trim().length > 0;

  /* ── Shared dark input styles ─────────────────────────── */

  const darkInputStyle: React.CSSProperties = {
    background: "rgba(255,255,255,0.06)",
    border: "1px solid rgba(255,255,255,0.1)",
    color: "#FAF9F8",
  };

  /* ── Render ───────────────────────────────────────────── */

  return (
    <div
      className="dark relative flex h-full w-full flex-col items-center overflow-hidden select-none"
      style={{
        background:
          "radial-gradient(ellipse 80% 60% at 50% 0%, #1C1210 0%, #0C0A09 55%, #080706 100%)",
        fontFamily: '"DM Sans", system-ui, sans-serif',
        color: "#FAF9F8",
      }}
    >
      {/* ── Decorative layers ───────────────────────────── */}

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

      {/* Ambient glow */}
      <div
        className="pointer-events-none absolute -top-40 left-1/2 -translate-x-1/2 w-[700px] h-[500px]"
        style={{
          background: "radial-gradient(ellipse, #E8623E, transparent 70%)",
          opacity: mounted ? 0.12 : 0,
          transition: "opacity 1.5s ease-out",
          animation: "onb-glow-pulse 6s ease-in-out infinite",
        }}
      />

      {/* ── WELCOME STEP ────────────────────────────────── */}

      {step === "welcome" && (
        <div className="flex flex-1 flex-col items-center justify-center w-full px-8">
          {/* Orbital container */}
          <div
            className="relative flex items-center justify-center"
            style={{
              width: 280,
              height: 280,
              ...anim(0),
            }}
          >
            {/* Orbit ring 1 */}
            <div
              className="absolute inset-0 rounded-full"
              style={{
                border: "1px solid rgba(232,98,62,0.12)",
                animation: "onb-orbit 25s linear infinite",
              }}
            >
              <div
                className="absolute rounded-full"
                style={{
                  width: 6,
                  height: 6,
                  background: "#E8623E",
                  top: -3,
                  left: "50%",
                  marginLeft: -3,
                  boxShadow: "0 0 10px #E8623E",
                }}
              />
            </div>

            {/* Orbit ring 2 */}
            <div
              className="absolute rounded-full"
              style={{
                width: 220,
                height: 220,
                top: 30,
                left: 30,
                border: "1px solid rgba(245,158,107,0.08)",
                animation: "onb-orbit-reverse 18s linear infinite",
              }}
            >
              <div
                className="absolute rounded-full"
                style={{
                  width: 4,
                  height: 4,
                  background: "#F59E6B",
                  bottom: -2,
                  left: "50%",
                  marginLeft: -2,
                  boxShadow: "0 0 8px #F59E6B",
                }}
              />
            </div>

            {/* Orbit ring 3 - outer */}
            <div
              className="absolute rounded-full"
              style={{
                width: 320,
                height: 320,
                top: -20,
                left: -20,
                border: "1px solid rgba(232,98,62,0.05)",
                animation: "onb-orbit 40s linear infinite",
              }}
            >
              <div
                className="absolute rounded-full"
                style={{
                  width: 3,
                  height: 3,
                  background: "rgba(248,180,160,0.5)",
                  top: "50%",
                  right: -1.5,
                  marginTop: -1.5,
                }}
              />
            </div>

            {/* Glow behind logo */}
            <div
              className="absolute rounded-full"
              style={{
                width: 140,
                height: 140,
                background:
                  "radial-gradient(circle, rgba(232,98,62,0.25), transparent 70%)",
                filter: "blur(20px)",
              }}
            />

            {/* Logo */}
            <img
              src="/app-icon.png"
              alt="Ossue"
              className="relative z-10"
              style={{
                width: 110,
                height: 110,
                animation:
                  "onb-scale-in 0.7s cubic-bezier(0.22,1,0.36,1) both",
                filter: "drop-shadow(0 0 30px rgba(232,98,62,0.3))",
              }}
            />
          </div>

          {/* Title */}
          <h1
            style={{
              ...anim(200),
              fontFamily: '"Syne", sans-serif',
              fontSize: "2.25rem",
              fontWeight: 700,
              letterSpacing: "-0.02em",
              lineHeight: 1.1,
              background:
                "linear-gradient(135deg, #FAF9F8 20%, #E8623E 60%, #F59E6B 100%)",
              WebkitBackgroundClip: "text",
              WebkitTextFillColor: "transparent",
              marginTop: 32,
            }}
          >
            Ossue
          </h1>

          {/* Tagline */}
          <p
            className="text-center max-w-sm"
            style={{
              ...anim(350),
              color: "rgba(250,249,248,0.65)",
              fontSize: "1rem",
              lineHeight: 1.6,
              marginTop: 14,
            }}
          >
            Your AI-powered inbox for open source maintenance.
            <br />
            Triage issues, review PRs, and respond faster.
          </p>

          {/* CTA Button */}
          <div style={anim(500)} className="mt-10">
            <button
              onClick={() => setStep("connect")}
              className="group relative flex items-center gap-2.5 rounded-xl px-8 py-3.5 font-semibold text-white transition-all duration-300 hover:scale-[1.03] active:scale-[0.98]"
              style={{
                background:
                  "linear-gradient(135deg, #E8623E 0%, #D94F2E 50%, #F59E6B 100%)",
                boxShadow:
                  "0 0 0 1px rgba(232,98,62,0.3), 0 4px 24px rgba(232,98,62,0.3), 0 1px 3px rgba(0,0,0,0.3)",
                fontSize: "0.95rem",
                fontFamily: '"Syne", sans-serif',
                letterSpacing: "0.01em",
              }}
            >
              Get Started
              <ArrowRight className="h-4 w-4 transition-transform duration-300 group-hover:translate-x-0.5" />
            </button>
          </div>

          {/* Subtle hint */}
          <p
            style={{
              ...anim(650),
              color: "rgba(250,249,248,0.3)",
              fontSize: "0.7rem",
              marginTop: 48,
              letterSpacing: "0.05em",
            }}
          >
            SETUP TAKES LESS THAN 2 MINUTES
          </p>
        </div>
      )}

      {/* ── FORM STEPS (connect / repos / ai / done) ───── */}

      {step !== "welcome" && (
        <>
          {/* Header: small logo + step indicator */}
          <div className="relative z-10 flex flex-col items-center pt-8 pb-4 gap-5">
            {/* Small logo */}
            <img
              src="/app-icon.png"
              alt=""
              style={{
                width: 36,
                height: 36,
                filter: "drop-shadow(0 0 12px rgba(232,98,62,0.25))",
                animation: "onb-fade-in 0.3s ease-out both",
              }}
            />

            {/* Step progress */}
            <div className="flex items-center gap-0.5">
              {STEPS.map((s, i) => {
                const isCompleted = i < stepIndex;
                const isActive = i === stepIndex;
                return (
                  <Fragment key={s.key}>
                    {i > 0 && (
                      <div
                        className="mx-1 transition-all duration-500"
                        style={{
                          width: 28,
                          height: 1,
                          background:
                            i <= stepIndex
                              ? "linear-gradient(90deg, #E8623E, #F59E6B)"
                              : "rgba(255,255,255,0.08)",
                        }}
                      />
                    )}
                    <button
                      type="button"
                      className="flex items-center gap-1.5"
                      style={{ cursor: isCompleted ? "pointer" : "default" }}
                      disabled={!isCompleted}
                      onClick={() => isCompleted && setStep(s.key)}
                    >
                      <div
                        className="rounded-full transition-all duration-500"
                        style={{
                          width: isActive ? 8 : 6,
                          height: isActive ? 8 : 6,
                          background: isCompleted
                            ? "#E8623E"
                            : isActive
                              ? "#F59E6B"
                              : "rgba(255,255,255,0.12)",
                          boxShadow: isActive
                            ? "0 0 10px rgba(232,98,62,0.5)"
                            : "none",
                        }}
                      />
                      <span
                        className="text-[11px] tracking-wider uppercase font-medium transition-colors duration-300"
                        style={{
                          color:
                            isCompleted || isActive
                              ? "rgba(250,249,248,0.8)"
                              : "rgba(255,255,255,0.2)",
                        }}
                      >
                        {s.label}
                      </span>
                    </button>
                  </Fragment>
                );
              })}
            </div>
          </div>

          {/* Content area */}
          <div className="flex flex-1 items-start justify-center w-full px-6 pt-2 pb-8 overflow-y-auto">
            <div
              key={step}
              className="w-full"
              style={{
                maxWidth: step === "ai" ? 740 : 520,
                animation:
                  "onb-fade-up 0.4s cubic-bezier(0.22,1,0.36,1) both",
              }}
            >
              {/* Glass card */}
              <div
                className="rounded-2xl p-8"
                style={{
                  background: "rgba(255,255,255,0.025)",
                  border: "1px solid rgba(255,255,255,0.06)",
                  backdropFilter: "blur(16px)",
                  boxShadow:
                    "0 8px 40px rgba(0,0,0,0.4), inset 0 1px 0 rgba(255,255,255,0.03)",
                }}
              >
                {/* ── CONNECT ───────────────────────────── */}
                {step === "connect" && (
                  <div className="space-y-6">
                    <div style={anim(50)}>
                      <h2
                        className="text-xl font-bold"
                        style={{
                          fontFamily: '"Syne", sans-serif',
                          color: "#FAF9F8",
                        }}
                      >
                        Connect your account
                      </h2>
                      <p
                        className="mt-1.5 text-sm leading-relaxed"
                        style={{ color: "rgba(250,249,248,0.6)" }}
                      >
                        Securely link a platform to start tracking your
                        projects.
                      </p>
                    </div>

                    {error && (
                      <p className="text-sm text-red-400 bg-red-500/10 border border-red-500/20 rounded-lg px-3 py-2">
                        {error}
                      </p>
                    )}

                    {/* Provider cards */}
                    <div className="grid grid-cols-2 gap-3" style={anim(150)}>
                      {(
                        [
                          {
                            key: "github" as const,
                            label: "GitHub",
                            Icon: Github,
                          },
                          {
                            key: "gitlab" as const,
                            label: "GitLab",
                            Icon: GitlabIcon,
                          },
                        ] as const
                      ).map(({ key, label, Icon }) => {
                        const selected = connectProvider === key;
                        return (
                          <button
                            key={key}
                            onClick={() => {
                              setConnectProvider(key);
                              setError(null);
                            }}
                            className="flex items-center gap-3 rounded-xl p-4 text-left transition-all duration-200"
                            style={{
                              background: selected
                                ? "rgba(232,98,62,0.08)"
                                : "rgba(255,255,255,0.02)",
                              border: selected
                                ? "1.5px solid rgba(232,98,62,0.4)"
                                : "1.5px solid rgba(255,255,255,0.06)",
                            }}
                          >
                            <div
                              className="flex h-10 w-10 items-center justify-center rounded-lg shrink-0"
                              style={{
                                background: selected
                                  ? "rgba(232,98,62,0.15)"
                                  : "rgba(255,255,255,0.04)",
                              }}
                            >
                              <Icon
                                className="h-5 w-5"
                                style={{
                                  color: selected
                                    ? "#F59E6B"
                                    : "rgba(250,249,248,0.4)",
                                }}
                              />
                            </div>
                            <div>
                              <span
                                className="text-sm font-semibold block"
                                style={{
                                  color: selected
                                    ? "#FAF9F8"
                                    : "rgba(250,249,248,0.5)",
                                }}
                              >
                                {label}
                              </span>
                              <span
                                className="text-xs"
                                style={{ color: "rgba(250,249,248,0.4)" }}
                              >
                                {key === "github"
                                  ? "OAuth or access token"
                                  : "Access token"}
                              </span>
                            </div>
                          </button>
                        );
                      })}
                    </div>

                    {/* Token input for selected provider */}
                    <div className="space-y-3" style={anim(250)}>
                      {connectProvider === "github" ? (
                        <>
                          <GitHubOAuthFlow onSuccess={handleOAuthSuccess} />

                          <div
                            className="flex items-center gap-3 my-4"
                          >
                            <div
                              className="flex-1 h-px"
                              style={{ background: "rgba(250,249,248,0.1)" }}
                            />
                            <span
                              className="text-xs font-medium uppercase tracking-wider"
                              style={{ color: "rgba(250,249,248,0.35)" }}
                            >
                              or
                            </span>
                            <div
                              className="flex-1 h-px"
                              style={{ background: "rgba(250,249,248,0.1)" }}
                            />
                          </div>

                          <div className="space-y-2">
                            <Label
                              htmlFor="github-token"
                              className="text-xs font-medium uppercase tracking-wider"
                              style={{ color: "rgba(250,249,248,0.55)" }}
                            >
                              GitHub Token
                            </Label>
                            <Input
                              id="github-token"
                              type="password"
                              placeholder="github_pat_... or ghp_..."
                              value={githubToken}
                              onChange={(e) => setGithubToken(e.target.value)}
                              className="h-11"
                            />
                            <p
                              className="text-xs leading-relaxed"
                              style={{ color: "rgba(250,249,248,0.45)" }}
                            >
                              Create a{" "}
                              <a
                                href="https://github.com/settings/personal-access-tokens/new?name=Ossue&description=Read-only+access+for+Ossue+app&issues=read&pull_requests=read"
                                target="_blank"
                                rel="noopener noreferrer"
                                className="inline-flex items-center gap-0.5 transition-colors"
                                style={{ color: "#F59E6B" }}
                              >
                                fine-grained personal access token
                                <ExternalLink className="h-3 w-3" />
                              </a>{" "}
                              with <strong>read-only</strong> access.
                            </p>
                          </div>

                          <Button
                            onClick={handleConnectGithub}
                            disabled={
                              !githubToken.trim() ||
                              savingProvider === "github"
                            }
                            className="w-full h-11 gap-2 rounded-xl font-semibold text-white border-0"
                            style={{
                              background:
                                "linear-gradient(135deg, #E8623E, #D94F2E)",
                              opacity:
                                !githubToken.trim() ||
                                savingProvider === "github"
                                  ? 0.5
                                  : 1,
                            }}
                          >
                            {savingProvider === "github" ? (
                              <Loader2 className="h-4 w-4 animate-spin" />
                            ) : (
                              <Github className="h-4 w-4" />
                            )}
                            Connect GitHub
                          </Button>
                        </>
                      ) : (
                        <>
                          <div className="space-y-2">
                            <Label
                              htmlFor="gitlab-token"
                              className="text-xs font-medium uppercase tracking-wider"
                              style={{ color: "rgba(250,249,248,0.55)" }}
                            >
                              GitLab Token
                            </Label>
                            <Input
                              id="gitlab-token"
                              type="password"
                              placeholder="glpat-..."
                              value={gitlabToken}
                              onChange={(e) => setGitlabToken(e.target.value)}
                              className="h-11"
                            />
                            <Input
                              id="gitlab-base-url"
                              type="text"
                              placeholder="https://gitlab.com (optional for self-hosted)"
                              value={gitlabBaseUrl}
                              onChange={(e) =>
                                setGitlabBaseUrl(e.target.value)
                              }
                              className="h-11"
                            />
                            <p
                              className="text-xs leading-relaxed"
                              style={{ color: "rgba(250,249,248,0.45)" }}
                            >
                              Create a{" "}
                              <a
                                href="https://gitlab.com/-/user_settings/personal_access_tokens?name=Ossue&scopes=read_api"
                                target="_blank"
                                rel="noopener noreferrer"
                                className="inline-flex items-center gap-0.5 transition-colors"
                                style={{ color: "#F59E6B" }}
                              >
                                personal access token
                                <ExternalLink className="h-3 w-3" />
                              </a>{" "}
                              with the <strong>read_api</strong> scope.
                            </p>
                          </div>

                          <Button
                            onClick={handleConnectGitlab}
                            disabled={
                              !gitlabToken.trim() ||
                              savingProvider === "gitlab"
                            }
                            className="w-full h-11 gap-2 rounded-xl font-semibold text-white border-0"
                            style={{
                              background:
                                "linear-gradient(135deg, #E8623E, #D94F2E)",
                              opacity:
                                !gitlabToken.trim() ||
                                savingProvider === "gitlab"
                                  ? 0.5
                                  : 1,
                            }}
                          >
                            {savingProvider === "gitlab" ? (
                              <Loader2 className="h-4 w-4 animate-spin" />
                            ) : (
                              <GitlabIcon className="h-4 w-4" />
                            )}
                            Connect GitLab
                          </Button>
                        </>
                      )}
                    </div>

                    {/* Security note */}
                    <div
                      className="flex items-start gap-2.5 rounded-xl p-3.5"
                      style={{
                        ...anim(350),
                        background: "rgba(255,255,255,0.02)",
                        border: "1px solid rgba(255,255,255,0.04)",
                      }}
                    >
                      <ShieldCheck
                        className="h-4 w-4 mt-0.5 shrink-0"
                        style={{ color: "rgba(245,158,107,0.5)" }}
                      />
                      <p
                        className="text-xs leading-relaxed"
                        style={{ color: "rgba(250,249,248,0.45)" }}
                      >
                        Your tokens are stored in a local encrypted database on
                        this device only. They are used exclusively to
                        communicate with GitHub/GitLab APIs.
                      </p>
                    </div>
                  </div>
                )}

                {/* ── REPOS ─────────────────────────────── */}
                {step === "repos" && (
                  <div className="space-y-6">
                    <div style={anim(50)}>
                      <h2
                        className="text-xl font-bold"
                        style={{
                          fontFamily: '"Syne", sans-serif',
                          color: "#FAF9F8",
                        }}
                      >
                        Select repositories
                      </h2>
                      <p
                        className="mt-1.5 text-sm"
                        style={{ color: "rgba(250,249,248,0.6)" }}
                      >
                        Choose which repos to track in your inbox.
                      </p>
                    </div>
                    <div style={anim(150)}>
                      <RepoPicker
                        connectors={connectors}
                        onDone={() => setStep("ai")}
                      />
                    </div>
                  </div>
                )}

                {/* ── AI ─────────────────────────────────── */}
                {step === "ai" && (
                  <div className="space-y-6">
                    <div style={anim(50)}>
                      <h2
                        className="text-xl font-bold"
                        style={{
                          fontFamily: '"Syne", sans-serif',
                          color: "#FAF9F8",
                        }}
                      >
                        Configure AI
                      </h2>
                      <p
                        className="mt-1.5 text-sm"
                        style={{ color: "rgba(250,249,248,0.6)" }}
                      >
                        Choose how the AI analyzes your projects. You can change
                        this later in Settings.
                      </p>
                    </div>

                    {error && (
                      <p className="text-sm text-red-400 bg-red-500/10 border border-red-500/20 rounded-lg px-3 py-2">
                        {error}
                      </p>
                    )}

                    {/* Provider cards — 3 column grid */}
                    <div className="grid grid-cols-3 gap-3" style={anim(150)}>
                      {AI_PROVIDERS.map((card) => {
                        const Icon = card.icon;
                        const selected = aiMode === card.mode;
                        return (
                          <button
                            key={card.mode}
                            onClick={() => setAiMode(card.mode)}
                            className="flex flex-col rounded-xl p-5 text-left transition-all duration-200"
                            style={{
                              background: selected
                                ? "rgba(232,98,62,0.08)"
                                : "rgba(255,255,255,0.02)",
                              border: selected
                                ? "2px solid rgba(232,98,62,0.4)"
                                : "2px solid rgba(255,255,255,0.06)",
                            }}
                          >
                            <div className="flex items-center gap-2.5 mb-3">
                              <div
                                className="flex h-9 w-9 items-center justify-center rounded-lg shrink-0"
                                style={{
                                  background: selected
                                    ? "rgba(232,98,62,0.15)"
                                    : "rgba(255,255,255,0.04)",
                                }}
                              >
                                <Icon
                                  className="h-4.5 w-4.5"
                                  style={{
                                    color: selected
                                      ? "#F59E6B"
                                      : "rgba(250,249,248,0.4)",
                                  }}
                                />
                              </div>
                              <span
                                className="text-sm font-semibold leading-tight"
                                style={{
                                  color: selected
                                    ? "#FAF9F8"
                                    : "rgba(250,249,248,0.55)",
                                }}
                              >
                                {card.title}
                              </span>
                            </div>
                            <p
                              className="text-xs mb-4"
                              style={{ color: "rgba(250,249,248,0.45)" }}
                            >
                              {card.description}
                            </p>
                            <div className="space-y-2 text-xs mt-auto">
                              {[
                                ["Speed", card.speed],
                                ["Review quality", card.reviewQuality],
                                ["Codebase", card.codebaseAccess],
                                ["Best for", card.bestFor],
                                ["Requires", card.requires],
                              ].map(([label, value]) => (
                                <div
                                  key={label}
                                  className="flex justify-between gap-2"
                                >
                                  <span
                                    style={{
                                      color: "rgba(250,249,248,0.35)",
                                    }}
                                  >
                                    {label}
                                  </span>
                                  <span
                                    className="font-medium text-right"
                                    style={{
                                      color: selected
                                        ? "rgba(250,249,248,0.85)"
                                        : "rgba(250,249,248,0.6)",
                                    }}
                                  >
                                    {value}
                                  </span>
                                </div>
                              ))}
                            </div>
                          </button>
                        );
                      })}
                    </div>

                    {/* Mode-specific config */}
                    {aiMode === "api" && (
                      <div className="space-y-2" style={anim(200)}>
                        <Label
                          className="text-xs font-medium uppercase tracking-wider"
                          style={{ color: "rgba(250,249,248,0.55)" }}
                        >
                          API Key <span style={{ color: "#E8623E" }}>*</span>
                        </Label>
                        <div className="flex gap-2">
                          <Input
                            type={showApiKey ? "text" : "password"}
                            placeholder="sk-ant-..."
                            value={aiApiKey}
                            onChange={(e) => setAiApiKey(e.target.value)}
                            className="h-11 flex-1"
                          />
                          <Button
                            variant="ghost"
                            size="icon"
                            className="shrink-0 h-11 w-11"
                            onClick={() => setShowApiKey(!showApiKey)}
                            style={{ color: "rgba(250,249,248,0.55)" }}
                          >
                            {showApiKey ? (
                              <EyeOff className="h-4 w-4" />
                            ) : (
                              <Eye className="h-4 w-4" />
                            )}
                          </Button>
                        </div>
                      </div>
                    )}

                    {aiMode !== "api" && (
                      <div
                        className="rounded-xl p-3.5"
                        style={{
                          ...anim(200),
                          background: "rgba(255,255,255,0.02)",
                          border: "1px solid rgba(255,255,255,0.04)",
                        }}
                      >
                        <p
                          className="text-xs leading-relaxed"
                          style={{ color: "rgba(250,249,248,0.5)" }}
                        >
                          {aiMode === "claude_cli"
                            ? "Claude Code CLI will be used for deep analysis. Make sure the 'claude' command is available in your PATH."
                            : "Cursor CLI will be used for deep analysis. Make sure the 'cursor' command is available in your PATH."}
                        </p>
                      </div>
                    )}

                    {/* Model */}
                    <div className="space-y-2" style={anim(250)}>
                      <Label
                        className="text-xs font-medium uppercase tracking-wider"
                        style={{ color: "rgba(250,249,248,0.55)" }}
                      >
                        Model
                      </Label>
                      <select
                        value={aiModel || ""}
                        onChange={(e) => setAiModel(e.target.value)}
                        className="w-full h-11 rounded-lg px-3 text-sm focus:outline-none focus:ring-0"
                        style={darkInputStyle}
                      >
                        <option value="">Auto (default)</option>
                        <option value="claude-sonnet-4-6">
                          Claude Sonnet 4.6
                        </option>
                        <option value="claude-opus-4-6">
                          Claude Opus 4.6
                        </option>
                        <option value="claude-haiku-4-5-20251001">
                          Claude Haiku 4.5
                        </option>
                      </select>
                    </div>

                    {/* Custom instructions */}
                    <div className="space-y-2" style={anim(300)}>
                      <Label
                        className="text-xs font-medium uppercase tracking-wider"
                        style={{ color: "rgba(250,249,248,0.55)" }}
                      >
                        Custom Instructions
                      </Label>
                      <textarea
                        className="w-full min-h-[90px] rounded-lg px-3 py-2.5 text-sm resize-y focus:outline-none focus:ring-0"
                        style={darkInputStyle}
                        placeholder="Add project-specific rules for the AI (e.g., 'We use conventional commits', 'All public APIs need JSDoc')"
                        value={aiCustomInstructions}
                        onChange={(e) =>
                          setAiCustomInstructions(e.target.value)
                        }
                        rows={3}
                      />
                    </div>

                    {/* Save — full width, no skip */}
                    <div style={anim(350)}>
                      <Button
                        onClick={handleSaveAiSettings}
                        disabled={isLoading || !canSaveAi}
                        className="w-full h-11 gap-2 rounded-xl font-semibold text-white border-0"
                        style={{
                          background:
                            "linear-gradient(135deg, #E8623E, #D94F2E)",
                          opacity: isLoading || !canSaveAi ? 0.5 : 1,
                        }}
                      >
                        {isLoading && (
                          <Loader2 className="h-4 w-4 animate-spin" />
                        )}
                        Save & Continue
                      </Button>
                      {aiMode === "api" && !aiApiKey.trim() && (
                        <p
                          className="text-xs text-center mt-2"
                          style={{ color: "rgba(250,249,248,0.4)" }}
                        >
                          An API key is required for API mode
                        </p>
                      )}
                    </div>
                  </div>
                )}

                {/* ── DONE ──────────────────────────────── */}
                {step === "done" && (
                  <div className="flex flex-col items-center text-center space-y-6 py-4">
                    {/* Celebration icon */}
                    <div className="relative" style={anim(0)}>
                      {/* Particle burst */}
                      {Array.from({ length: 8 }).map((_, i) => {
                        const angle = (i / 8) * 360;
                        const rad = (angle * Math.PI) / 180;
                        const dist = 35 + Math.random() * 20;
                        return (
                          <div
                            key={i}
                            className="absolute left-1/2 top-1/2 rounded-full"
                            style={{
                              width: 4,
                              height: 4,
                              marginLeft: -2,
                              marginTop: -2,
                              background:
                                i % 2 === 0 ? "#E8623E" : "#F59E6B",
                              ["--px" as string]: `${Math.cos(rad) * dist}px`,
                              ["--py" as string]: `${Math.sin(rad) * dist}px`,
                              animation: `onb-particle 0.8s cubic-bezier(0.22,1,0.36,1) ${200 + i * 50}ms both`,
                            }}
                          />
                        );
                      })}

                      <div
                        className="flex h-20 w-20 items-center justify-center rounded-2xl"
                        style={{
                          background:
                            "linear-gradient(135deg, rgba(232,98,62,0.15), rgba(245,158,107,0.1))",
                          border: "1px solid rgba(232,98,62,0.2)",
                        }}
                      >
                        <Sparkles
                          className="h-9 w-9"
                          style={{ color: "#F59E6B" }}
                        />
                      </div>
                    </div>

                    <div style={anim(200)}>
                      <h2
                        className="text-2xl font-bold"
                        style={{
                          fontFamily: '"Syne", sans-serif',
                          background:
                            "linear-gradient(135deg, #FAF9F8, #F59E6B)",
                          WebkitBackgroundClip: "text",
                          WebkitTextFillColor: "transparent",
                        }}
                      >
                        You're all set!
                      </h2>
                      <p
                        className="mt-2 text-sm"
                        style={{ color: "rgba(250,249,248,0.6)" }}
                      >
                        Your inbox is ready. Start triaging your open source
                        projects.
                      </p>
                    </div>

                    <div style={anim(400)}>
                      <button
                        onClick={handleFinish}
                        className="group flex items-center gap-2.5 rounded-xl px-8 py-3.5 font-semibold text-white transition-all duration-300 hover:scale-[1.03] active:scale-[0.98]"
                        style={{
                          background:
                            "linear-gradient(135deg, #E8623E 0%, #D94F2E 50%, #F59E6B 100%)",
                          boxShadow:
                            "0 0 0 1px rgba(232,98,62,0.3), 0 4px 24px rgba(232,98,62,0.3), 0 1px 3px rgba(0,0,0,0.3)",
                          fontFamily: '"Syne", sans-serif',
                          fontSize: "0.95rem",
                          letterSpacing: "0.01em",
                        }}
                      >
                        Open Inbox
                        <ArrowRight className="h-4 w-4 transition-transform duration-300 group-hover:translate-x-0.5" />
                      </button>
                    </div>
                  </div>
                )}
              </div>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
