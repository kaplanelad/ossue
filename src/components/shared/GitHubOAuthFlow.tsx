import { useState, useEffect, useRef, useCallback } from "react";
import { Button } from "@/components/ui/button";
import { Loader2, Github, Copy, Check, ExternalLink } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import * as api from "@/lib/tauri";
import { errorMessage } from "@/lib/utils";

type OAuthFlowState =
  | { step: "idle" }
  | { step: "starting" }
  | {
      step: "waiting";
      userCode: string;
      verificationUri: string;
    }
  | { step: "success"; accessToken: string }
  | { step: "expired" }
  | { step: "denied" }
  | { step: "error"; message: string };

interface GitHubOAuthFlowProps {
  onSuccess: (token: string) => void;
  onCancel?: () => void;
  /** Compact mode for dialogs */
  compact?: boolean;
}

export function GitHubOAuthFlow({
  onSuccess,
  onCancel,
  compact,
}: GitHubOAuthFlowProps) {
  const [state, setState] = useState<OAuthFlowState>({ step: "idle" });
  const [copied, setCopied] = useState(false);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const cleanup = useCallback(() => {
    if (pollRef.current) {
      clearInterval(pollRef.current);
      pollRef.current = null;
    }
  }, []);

  useEffect(() => {
    return () => {
      cleanup();
      api.cancelGithubOAuth().catch(() => {});
    };
  }, [cleanup]);

  const startFlow = async () => {
    setState({ step: "starting" });
    try {
      const resp = await api.startGithubOAuth();
      setState({
        step: "waiting",
        userCode: resp.user_code,
        verificationUri: resp.verification_uri,
      });

      // Start polling
      const intervalMs = (resp.interval || 5) * 1000;
      pollRef.current = setInterval(async () => {
        try {
          const poll = await api.pollGithubOAuth();
          switch (poll.status) {
            case "success":
              cleanup();
              setState({ step: "success", accessToken: poll.access_token! });
              onSuccess(poll.access_token!);
              break;
            case "expired":
              cleanup();
              setState({ step: "expired" });
              break;
            case "denied":
              cleanup();
              setState({ step: "denied" });
              break;
            case "slow_down":
              // GitHub wants us to slow down — clear and restart with longer interval
              cleanup();
              pollRef.current = setInterval(async () => {
                try {
                  const p = await api.pollGithubOAuth();
                  if (p.status === "success") {
                    cleanup();
                    setState({ step: "success", accessToken: p.access_token! });
                    onSuccess(p.access_token!);
                  } else if (
                    p.status === "expired" ||
                    p.status === "denied"
                  ) {
                    cleanup();
                    setState({ step: p.status });
                  }
                } catch {
                  // ignore poll errors
                }
              }, intervalMs + 5000);
              break;
            case "error":
              cleanup();
              setState({
                step: "error",
                message: "An error occurred during authorization",
              });
              break;
            // "pending" - keep polling
          }
        } catch {
          // ignore transient poll errors
        }
      }, intervalMs);
    } catch (err) {
      setState({ step: "error", message: errorMessage(err) });
    }
  };

  const handleCancel = async () => {
    cleanup();
    await api.cancelGithubOAuth().catch(() => {});
    setState({ step: "idle" });
    onCancel?.();
  };

  const handleCopyCode = async (code: string) => {
    await navigator.clipboard.writeText(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleRetry = () => {
    setState({ step: "idle" });
    startFlow();
  };

  // Idle state — show the sign in button
  if (state.step === "idle") {
    return (
      <Button
        onClick={startFlow}
        variant="outline"
        className={compact ? "w-full gap-2" : "w-full h-11 gap-2 rounded-xl"}
      >
        <Github className="h-4 w-4" />
        Sign in with GitHub
      </Button>
    );
  }

  // Starting
  if (state.step === "starting") {
    return (
      <div className="flex items-center justify-center gap-2 py-3">
        <Loader2 className="h-4 w-4 animate-spin" />
        <span className="text-sm text-muted-foreground">
          Starting GitHub sign in...
        </span>
      </div>
    );
  }

  // Waiting for user to authorize
  if (state.step === "waiting") {
    return (
      <div className="space-y-3">
        <div className="text-center space-y-2">
          <p className="text-sm text-muted-foreground">
            Enter this code on GitHub:
          </p>
          <button
            onClick={() => handleCopyCode(state.userCode)}
            className="inline-flex items-center gap-2 rounded-lg border px-4 py-2 font-mono text-xl font-bold tracking-widest transition-colors hover:bg-accent"
          >
            {state.userCode}
            {copied ? (
              <Check className="h-4 w-4 text-green-500" />
            ) : (
              <Copy className="h-4 w-4 text-muted-foreground" />
            )}
          </button>
        </div>

        <Button
          onClick={() => openUrl(state.verificationUri)}
          className={
            compact
              ? "w-full gap-2"
              : "w-full h-11 gap-2 rounded-xl font-semibold text-white border-0"
          }
          style={
            compact
              ? undefined
              : {
                  background: "linear-gradient(135deg, #E8623E, #D94F2E)",
                }
          }
          variant={compact ? "default" : undefined}
        >
          <ExternalLink className="h-4 w-4" />
          Open GitHub
        </Button>

        <div className="flex items-center justify-center gap-2 py-1">
          <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />
          <span className="text-xs text-muted-foreground">
            Waiting for authorization...
          </span>
        </div>

        <Button
          onClick={handleCancel}
          variant="ghost"
          size="sm"
          className="w-full text-xs text-muted-foreground"
        >
          Cancel
        </Button>
      </div>
    );
  }

  // Success — handled by parent via onSuccess, but show briefly
  if (state.step === "success") {
    return (
      <div className="flex items-center justify-center gap-2 py-3">
        <Check className="h-4 w-4 text-green-500" />
        <span className="text-sm text-green-500">
          GitHub connected successfully!
        </span>
      </div>
    );
  }

  // Expired
  if (state.step === "expired") {
    return (
      <div className="space-y-3">
        <p className="text-sm text-center text-yellow-500">
          The authorization code expired. Please try again.
        </p>
        <Button onClick={handleRetry} variant="outline" className="w-full gap-2">
          <Github className="h-4 w-4" />
          Try again
        </Button>
      </div>
    );
  }

  // Denied
  if (state.step === "denied") {
    return (
      <div className="space-y-3">
        <p className="text-sm text-center text-red-400">
          Authorization was denied. Please try again.
        </p>
        <Button onClick={handleRetry} variant="outline" className="w-full gap-2">
          <Github className="h-4 w-4" />
          Try again
        </Button>
      </div>
    );
  }

  // Error
  return (
    <div className="space-y-3">
      <p className="text-sm text-center text-red-400">{state.message}</p>
      <Button onClick={handleRetry} variant="outline" className="w-full gap-2">
        <Github className="h-4 w-4" />
        Try again
      </Button>
    </div>
  );
}
