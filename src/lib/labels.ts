export const LABEL_OPTIONS = [
  { value: "bug", color: "oklch(0.577 0.245 27)" },
  { value: "feature", color: "oklch(0.488 0.243 264)" },
  { value: "enhancement", color: "oklch(0.6 0.118 184)" },
  { value: "documentation", color: "oklch(0.828 0.189 84)" },
  { value: "performance", color: "oklch(0.646 0.222 41)" },
  { value: "security", color: "oklch(0.577 0.245 27)" },
  { value: "refactor", color: "oklch(0.627 0.265 303)" },
  { value: "testing", color: "oklch(0.6 0.118 184)" },
  { value: "ui", color: "oklch(0.769 0.188 70)" },
  { value: "api", color: "oklch(0.488 0.243 264)" },
  { value: "database", color: "oklch(0.398 0.07 227)" },
  { value: "ci-cd", color: "oklch(0.645 0.246 16)" },
];

export function getLabelColor(label: string): string | undefined {
  return LABEL_OPTIONS.find((l) => l.value === label)?.color;
}
