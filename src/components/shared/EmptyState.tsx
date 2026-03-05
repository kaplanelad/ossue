import type { LucideIcon } from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";

interface EmptyStateAction {
  label: string;
  onClick: () => void;
  icon?: LucideIcon;
}

interface EmptyStateProps {
  icon: LucideIcon;
  iconClassName?: string;
  iconContainerClassName?: string;
  title: string;
  description?: string;
  action?: EmptyStateAction;
}

export function EmptyState({
  icon: Icon,
  iconClassName,
  iconContainerClassName,
  title,
  description,
  action,
}: EmptyStateProps) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-4 p-8">
      {/* Orbital container — smaller version of onboarding welcome */}
      <div className="relative flex items-center justify-center" style={{ width: 140, height: 140 }}>
        {/* Orbit ring 1 */}
        <div
          className="absolute inset-0 rounded-full border border-primary/10"
          style={{ animation: "onb-orbit 25s linear infinite" }}
        >
          <div
            className="absolute rounded-full bg-primary"
            style={{
              width: 4,
              height: 4,
              top: -2,
              left: "50%",
              marginLeft: -2,
              boxShadow: "0 0 8px hsl(var(--primary))",
            }}
          />
        </div>

        {/* Orbit ring 2 */}
        <div
          className="absolute rounded-full border border-primary/[0.06]"
          style={{
            width: 110,
            height: 110,
            top: 15,
            left: 15,
            animation: "onb-orbit-reverse 18s linear infinite",
          }}
        >
          <div
            className="absolute rounded-full bg-primary/70"
            style={{
              width: 3,
              height: 3,
              bottom: -1.5,
              left: "50%",
              marginLeft: -1.5,
              boxShadow: "0 0 6px hsl(var(--primary) / 0.5)",
            }}
          />
        </div>

        {/* Glow behind icon */}
        <div
          className="absolute rounded-full bg-primary/15 blur-xl"
          style={{ width: 70, height: 70 }}
        />

        {/* Icon container */}
        <div
          className={cn(
            "relative z-10 flex h-14 w-14 items-center justify-center rounded-2xl bg-muted shadow-sm",
            iconContainerClassName
          )}
          style={{
            animation: "onb-scale-in 0.5s cubic-bezier(0.22,1,0.36,1) both",
          }}
        >
          <Icon className={cn("h-6 w-6 text-muted-foreground/70", iconClassName)} />
        </div>
      </div>

      {/* Text */}
      <div
        className="flex flex-col items-center gap-1.5"
        style={{
          animation: "onb-fade-up 0.5s cubic-bezier(0.22,1,0.36,1) both",
          animationDelay: "150ms",
        }}
      >
        <p className="text-sm font-medium text-foreground">{title}</p>
        {description && (
          <p className="text-xs text-center text-muted-foreground max-w-[240px]">
            {description}
          </p>
        )}
      </div>

      {/* Action */}
      {action && (
        <div
          style={{
            animation: "onb-fade-up 0.5s cubic-bezier(0.22,1,0.36,1) both",
            animationDelay: "300ms",
          }}
        >
          <Button variant="outline" size="sm" onClick={action.onClick}>
            {action.icon && <action.icon className="h-3.5 w-3.5" />}
            {action.label}
          </Button>
        </div>
      )}
    </div>
  );
}
