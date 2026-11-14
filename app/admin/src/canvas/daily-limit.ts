// D10 (trial-25 plan): once the app's daily model-call cap is reached, every
// model route answers 429 `daily_limit` (`composer::model::daily_limit`).
// The canvas says so in these words wherever an AI request was refused.
export const DAILY_LIMIT_CODE = 'daily_limit';
export const DAILY_LIMIT_MESSAGE = "Today's AI limit for this workspace is reached. Building apps and Ask pause until midnight UTC; your apps and entries keep working.";
