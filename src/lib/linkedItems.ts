import type { Item } from "@/types";

/** Regex matching GitHub closing keywords: Closes #123, Fixes #456, Resolves #789 */
const CLOSING_KEYWORD_RE = /(?:close[sd]?|fix(?:e[sd])?|resolve[sd]?)\s+#(\d+)/gi;

/** Extract issue numbers referenced via closing keywords in text */
export function extractLinkedIssueNumbers(body: string): number[] {
  const numbers = new Set<number>();
  let match;
  while ((match = CLOSING_KEYWORD_RE.exec(body)) !== null) {
    numbers.add(parseInt(match[1], 10));
  }
  return Array.from(numbers);
}

/** Find items linked to the given item within the same project */
export function findLinkedItems(item: Item, allItems: Item[]): Item[] {
  if (item.type_data.kind === "note") return [];

  const projectItems = allItems.filter(
    (i) => i.project_id === item.project_id && i.id !== item.id && i.type_data.kind !== "note"
  );

  if (item.item_type === "pr") {
    // PR → find linked issues by parsing PR body for closing keywords
    const linkedNumbers = extractLinkedIssueNumbers(item.body);
    if (linkedNumbers.length === 0) return [];
    const numSet = new Set(linkedNumbers);
    return projectItems.filter(
      (i) => i.type_data.kind !== "note" && numSet.has(i.type_data.external_id)
    );
  }

  if (item.item_type === "issue") {
    // Issue → find PRs that reference this issue number
    const issueNumber = item.type_data.external_id;
    return projectItems.filter((i) => {
      if (i.item_type !== "pr") return false;
      const refs = extractLinkedIssueNumbers(i.body);
      return refs.includes(issueNumber);
    });
  }

  return [];
}
