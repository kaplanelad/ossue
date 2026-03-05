import type { ReactNode } from "react";

interface SettingHeaderProps {
  title: string;
  subtitle: string;
  action?: ReactNode;
}

export function SettingHeader({ title, subtitle, action }: SettingHeaderProps) {
  return (
    <div className="flex items-center justify-between">
      <div>
        <h3 className="text-sm font-bold tracking-tight">{title}</h3>
        <p className="text-xs text-muted-foreground">{subtitle}</p>
      </div>
      {action && <div>{action}</div>}
    </div>
  );
}
