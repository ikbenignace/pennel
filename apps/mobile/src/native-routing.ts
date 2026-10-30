export function nativeSystemPath(path: string, schemes: readonly string[]): string {
  try {
    const url = new URL(path);
    const ownScheme = schemes.includes(url.protocol.slice(0, -1));
    const sync = (url.hostname === 'sync' && (url.pathname === '' || url.pathname === '/')) || (!url.hostname && url.pathname === '/sync');
    if (ownScheme && sync && !url.username && !url.password && !url.search && !url.hash) return '/settings/sync';
  } catch {}
  return path;
}
