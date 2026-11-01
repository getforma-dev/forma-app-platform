// Native PageProps contract. JSON refresh responses use Rust's snake_case.
export type Cell = { id: string; text: string; raw: string };
export type Pair = { id: string; label: string; value: string; raw: string };
export type View = {
  id: string; title: string; kind: string; state: string; badge: string; message: string;
  idle: boolean; loading: boolean; ready: boolean; empty: boolean; failed: boolean; denied: boolean;
  hasSummary: boolean; summary: Pair[]; hasTable: boolean; columns: Cell[]; rows: { id: string; cells: Cell[] }[];
  hasReceipt: boolean; receiptDigest: string; receipt: Pair[]; receiptJson: string;
};
export type RecordRow = { id: string; version: string; committedAt: string; commitSequence: string; cells: Cell[] };
export type RecordsCard = {
  id: string; title: string; summary: string; collection: string; hasRows: boolean; hasNoRows: boolean;
  columns: Cell[]; rows: RecordRow[];
};
export type SharedPageProps = {
  title: string; shareId: string; status: string; ownerLabel: string; expiresAt: string;
  canAddToCanvas: boolean; addedToCanvas: boolean; available: boolean; message: string;
  needsSignIn: boolean; canSignUp: boolean; canAccept: boolean; signInAction: string; signInCsrf: string;
  canSignOut: boolean; csrfSignout: string;
  views: View[]; recordCards: RecordsCard[]; recordsTruncated: boolean;
};
export type SharedApp = {
  id: string; title: string; href: string; ownerLabel: string; canAddToCanvas: boolean;
  views: View[]; recordCards: RecordsCard[]; recordsTruncated: boolean;
};
export type SharedCanvasProps = { accountLabel: string; csrfSignout: string; canSignOut: boolean; hasWorkspace: boolean; hasApps: boolean; apps: SharedApp[] };
