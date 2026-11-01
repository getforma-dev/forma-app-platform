/** Closed numeric layout only. Collisions flow forward and never cover a part. */
export type PartSlot = { id: string; row: number; column: number; width: number };
export function declaredLayout(ids: string[], json: string): PartSlot[] | null {
  let input: unknown; try { input = JSON.parse(json); } catch { return null; }
  if (!Array.isArray(input) || input.length > 100) return null;
  const wanted = new Set(ids), seen = new Set<string>(), rows: PartSlot[] = [];
  let row = 0, previous: number | null = null, occupied = 0;
  const values: PartSlot[] = [];
  for (const raw of input) {
    if (!raw || typeof raw !== 'object') return null;
    const part = raw as PartSlot;
    if (typeof part.id !== 'string' || seen.has(part.id) || ![part.row,part.column,part.width].every(Number.isSafeInteger)
      || part.row < 0 || part.row > 4294967295 || part.column < 0 || part.column > 11 || part.width < 1 || part.width > 12) return null;
    seen.add(part.id); if (wanted.has(part.id)) values.push(part);
  }
  values.sort((a,b) => a.row-b.row || a.column-b.column);
  for (const part of values) {
    const width = Math.min(part.width, 12-part.column), mask = ((1 << width)-1) << part.column;
    if (previous !== part.row || (occupied & mask) !== 0) { row++; occupied=0; }
    previous=part.row; occupied |= mask;
    rows.push({id:part.id,row,column:part.column,width});
  }
  // App controls/current-record utilities are supplemental, never silently lost.
  for (const id of ids) if (!seen.has(id)) rows.push({id,row:++row,column:0,width:12});
  return rows;
}
