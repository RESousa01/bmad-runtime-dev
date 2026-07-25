import type { WorkspaceSearchMatch } from "./hostClient";

const stopWords = new Set([
  "about",
  "after",
  "again",
  "also",
  "and",
  "before",
  "could",
  "files",
  "for",
  "from",
  "have",
  "into",
  "please",
  "review",
  "should",
  "that",
  "the",
  "their",
  "this",
  "with",
  "would",
]);

/**
 * Extracts a small deterministic set of literal terms for the host's bounded,
 * read-only workspace search. This is local suggestion logic, not a model call.
 */
export function contextSearchTerms(intent: string, maxTerms = 6): string[] {
  if (!Number.isInteger(maxTerms) || maxTerms < 1) return [];
  const terms: string[] = [];
  const seen = new Set<string>();
  for (const match of intent.toLocaleLowerCase("en-US").matchAll(/[\p{L}\p{N}_-]{3,}/gu)) {
    const term = match[0];
    if (stopWords.has(term) || seen.has(term)) continue;
    seen.add(term);
    terms.push(term);
    if (terms.length === maxTerms) break;
  }
  return terms;
}

/**
 * Ranks one result group per search term. A file scores at most once per term,
 * which prevents a repetitive file from crowding out cross-term relevance.
 */
export function rankContextPaths(
  resultGroups: readonly (readonly WorkspaceSearchMatch[])[],
  maxPaths = 8,
): string[] {
  if (!Number.isInteger(maxPaths) || maxPaths < 1) return [];
  const scores = new Map<string, number>();
  resultGroups.forEach((matches, groupIndex) => {
    const uniquePaths = new Set(matches.map((match) => match.relativePath));
    const weight = resultGroups.length - groupIndex;
    for (const relativePath of uniquePaths) {
      scores.set(relativePath, (scores.get(relativePath) ?? 0) + weight);
    }
  });
  return [...scores]
    .sort(([leftPath, leftScore], [rightPath, rightScore]) =>
      rightScore - leftScore || leftPath.localeCompare(rightPath, "en-US"))
    .slice(0, maxPaths)
    .map(([relativePath]) => relativePath);
}
