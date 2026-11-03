/** An exact saved-work label; shell-wide library names use the existing labels API. */
export type WorkName = { name: string | null; version: number; can_name: boolean; scope: 'work' | 'library' };
const unavailable = 'This app’s name is unavailable. Your draft is kept; try again.';
function label(value: unknown): WorkName | null {
  if (!value || typeof value !== 'object') return null;
  const item = value as WorkName;
  return (item.name === null || typeof item.name === 'string') && Number.isSafeInteger(item.version) && item.version >= 0
    && typeof item.can_name === 'boolean' && (item.scope === 'work' || item.scope === 'library') ? item : null;
}
export async function readWorkName(app: string, work: string): Promise<WorkName | string> {
  try {
    const response = await fetch(`/v1/work/${app}/${work}/label`, { credentials: 'same-origin', cache: 'no-store' });
    if (!response.ok) return response.status === 404 ? 'You cannot name this app.' : unavailable;
    return label(await response.json()) ?? unavailable;
  } catch { return unavailable; }
}
export async function saveWorkName(app: string, work: string, name: string, version: number): Promise<WorkName | string> {
  try {
    const response = await fetch(`/v1/work/${app}/${work}/label`, {
      method: 'POST', credentials: 'same-origin', cache: 'no-store', headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ name, expected_version: version }),
    });
    if (response.status === 409) return 'This app’s name changed elsewhere. Your draft is kept. Reopen the editor to review the current name before saving.';
    if (!response.ok) return response.status === 404 ? 'You cannot name this app.' : unavailable;
    const json = await response.json() as { saved?: unknown; label?: unknown };
    const next = label(json.label);
    const confirmed = next?.scope === 'work' && next.name === name
      && ((json.saved === true && next.version === version + 1) || (json.saved === false && next.version === version));
    return confirmed ? next! : unavailable;
  } catch { return unavailable; }
}
