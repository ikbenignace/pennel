export type ContentHeadingTag = "h2" | "h3" | "h4" | "h5" | "h6";

/** The containing meeting owns h1; generated Markdown can start at any source depth. */
export function contentHeadingTags(depths: readonly number[]): ContentHeadingTag[] {
  const ancestors: number[] = [];
  const tags: ContentHeadingTag[] = ["h2", "h3", "h4", "h5", "h6"];
  return depths.map((depth) => {
    while (ancestors.length && ancestors[ancestors.length - 1] >= depth) ancestors.pop();
    ancestors.push(depth);
    return tags[Math.min(ancestors.length, tags.length) - 1];
  });
}
