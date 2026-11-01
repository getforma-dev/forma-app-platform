// Closed work/app identifiers shared by the document and canvas transports.
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
export function isUuid(value: unknown): value is string {
  return typeof value === 'string' && UUID.test(value);
}
