import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const AI_FOCUS_AREAS = [
  { key: "security", label: "Security" },
  { key: "performance", label: "Performance" },
  { key: "api_compatibility", label: "API Compatibility" },
  { key: "test_coverage", label: "Test Coverage" },
  { key: "code_style", label: "Code Style" },
  { key: "documentation", label: "Documentation" },
];

interface Props {
  focusAreas: string[];
  reviewStrictness: string;
  responseTone: string;
  onFocusAreasChange: (areas: string[]) => void;
  onReviewStrictnessChange: (strictness: string) => void;
  onResponseToneChange: (tone: string) => void;
  /** Per-project mode: show "Use global default" toggles */
  projectMode?: boolean;
  /** Override flags for each preference in project mode */
  overrides?: {
    focusAreas: boolean;
    reviewStrictness: boolean;
    responseTone: boolean;
  };
  onOverrideChange?: (key: string, enabled: boolean) => void;
  /** Global default values shown as hints in project mode */
  globalDefaults?: {
    focusAreas: string[];
    reviewStrictness: string;
    responseTone: string;
  };
}

export function AIPreferencesForm({
  focusAreas,
  reviewStrictness,
  responseTone,
  onFocusAreasChange,
  onReviewStrictnessChange,
  onResponseToneChange,
  projectMode = false,
  overrides,
  onOverrideChange,
  globalDefaults,
}: Props) {
  const toggleFocusArea = (areaKey: string, checked: boolean) => {
    const next = checked
      ? [...focusAreas, areaKey]
      : focusAreas.filter((a) => a !== areaKey);
    onFocusAreasChange(next);
  };

  const strictnessLabels: Record<string, string> = {
    strict: "Strict",
    pragmatic: "Pragmatic",
    lenient: "Lenient",
  };

  const toneLabels: Record<string, string> = {
    friendly: "Friendly",
    neutral: "Neutral",
    terse: "Terse",
  };

  return (
    <div className="space-y-6">
      {/* Analysis Focus Areas */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <div>
            <Label className="text-sm font-semibold">Analysis Focus Areas</Label>
            <p className="text-xs text-muted-foreground">
              Select which aspects the AI should prioritize
            </p>
          </div>
          {projectMode && onOverrideChange && (
            <label className="flex items-center gap-2 text-xs">
              <input
                type="checkbox"
                checked={!overrides?.focusAreas}
                onChange={(e) => onOverrideChange("ai_focus_areas", !e.target.checked)}
                className="h-3.5 w-3.5 rounded border-input"
              />
              Use global default
            </label>
          )}
        </div>
        {projectMode && !overrides?.focusAreas && globalDefaults && (
          <p className="text-xs text-muted-foreground italic">
            Global: {globalDefaults.focusAreas.join(", ") || "all"}
          </p>
        )}
        <div className={`grid grid-cols-2 gap-3 ${projectMode && !overrides?.focusAreas ? "opacity-50 pointer-events-none" : ""}`}>
          {AI_FOCUS_AREAS.map((area) => (
            <label
              key={area.key}
              className="flex items-center gap-3 rounded-lg border p-3 cursor-pointer hover:bg-accent/50 transition-colors"
            >
              <input
                type="checkbox"
                checked={focusAreas.includes(area.key)}
                onChange={(e) => toggleFocusArea(area.key, e.target.checked)}
                className="h-4 w-4 rounded border-input"
              />
              <span className="text-sm font-medium">{area.label}</span>
            </label>
          ))}
        </div>
      </div>

      {/* Review Strictness */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <div>
            <Label className="text-sm font-semibold">Review Strictness</Label>
            <p className="text-xs text-muted-foreground">
              How strict the AI should be when reviewing code
            </p>
          </div>
          {projectMode && onOverrideChange && (
            <label className="flex items-center gap-2 text-xs">
              <input
                type="checkbox"
                checked={!overrides?.reviewStrictness}
                onChange={(e) => onOverrideChange("ai_review_strictness", !e.target.checked)}
                className="h-3.5 w-3.5 rounded border-input"
              />
              Use global default
            </label>
          )}
        </div>
        {projectMode && !overrides?.reviewStrictness && globalDefaults && (
          <p className="text-xs text-muted-foreground italic">
            Global: {strictnessLabels[globalDefaults.reviewStrictness] || globalDefaults.reviewStrictness}
          </p>
        )}
        <div className={projectMode && !overrides?.reviewStrictness ? "opacity-50 pointer-events-none" : ""}>
          <Select
            value={reviewStrictness}
            onValueChange={onReviewStrictnessChange}
          >
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="strict">Strict — flag everything</SelectItem>
              <SelectItem value="pragmatic">Pragmatic — focus on blockers</SelectItem>
              <SelectItem value="lenient">Lenient — suggestions only</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>

      {/* Response Tone */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <div>
            <Label className="text-sm font-semibold">Response Tone</Label>
            <p className="text-xs text-muted-foreground">
              Tone of AI-generated responses
            </p>
          </div>
          {projectMode && onOverrideChange && (
            <label className="flex items-center gap-2 text-xs">
              <input
                type="checkbox"
                checked={!overrides?.responseTone}
                onChange={(e) => onOverrideChange("ai_response_tone", !e.target.checked)}
                className="h-3.5 w-3.5 rounded border-input"
              />
              Use global default
            </label>
          )}
        </div>
        {projectMode && !overrides?.responseTone && globalDefaults && (
          <p className="text-xs text-muted-foreground italic">
            Global: {toneLabels[globalDefaults.responseTone] || globalDefaults.responseTone}
          </p>
        )}
        <div className={projectMode && !overrides?.responseTone ? "opacity-50 pointer-events-none" : ""}>
          <Select
            value={responseTone}
            onValueChange={onResponseToneChange}
          >
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="friendly">Friendly</SelectItem>
              <SelectItem value="neutral">Neutral</SelectItem>
              <SelectItem value="terse">Terse</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>
    </div>
  );
}
