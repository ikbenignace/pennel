export async function saveThenRename(save: () => Promise<boolean>, rename: () => Promise<string | undefined>): Promise<string | undefined> {
  if (!(await save())) return undefined;
  return rename();
}
