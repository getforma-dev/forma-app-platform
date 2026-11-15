// D10 (trial-25 plan): once the app's daily model-call cap is reached, every
// model route answers 429 `daily_limit` (`composer::model::daily_limit`).
// The canvas says so in these words wherever an AI request was refused.
export const DAILY_LIMIT_CODE = 'daily_limit';
export const DAILY_LIMIT_MESSAGE = "Today's AI limit for this workspace is reached. Building apps and Ask pause until midnight UTC; your apps and entries keep working.";
// APP-215 (content M3): the same code with `"scope":"member"` is the member's
// own daily allowance (`composer::model::daily_limit_for`), so the words say
// it is theirs and that nobody else is paused.
export const MEMBER_DAILY_LIMIT_MESSAGE = "You've used your AI allowance for today. Building apps and Ask resume for you at 00:00 UTC; your apps and entries keep working.";
const object = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value);
/** Whether a problem body is the daily-limit refusal. */
export function isDailyLimit(body: unknown): boolean {
  return object(body) && body.code === DAILY_LIMIT_CODE;
}
/** The daily-limit sentence for a problem body: the member's own when the server scoped it to them, else the workspace's. */
export function dailyLimitMessage(body: unknown): string {
  return object(body) && body.scope === 'member' ? MEMBER_DAILY_LIMIT_MESSAGE : DAILY_LIMIT_MESSAGE;
}
