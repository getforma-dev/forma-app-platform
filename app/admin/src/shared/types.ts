import type { RecordComponent } from '../record-components/types';
import type { PresentationProps } from '../presentation/presentation';
// Native PageProps contract. JSON refresh responses use Rust's snake_case.
export type Cell = { id: string; text: string; raw: string };
export type Pair = { id: string; label: string; value: string; raw: string };
export type View = {
  id: string; title: string; kind: string; state: string; badge: string; message: string;
  idle: boolean; loading: boolean; ready: boolean; empty: boolean; failed: boolean; denied: boolean;
  hasSummary: boolean; summary: Pair[]; hasTable: boolean; columns: Cell[]; rows: { id: string; cells: Cell[] }[];
  hasChart: boolean; chart: {
    label: string; caption: string; viewBox: string; hasBars: boolean; hasLine: boolean; line: string; area: string;
    bars: { id: string; x: string; y: string; width: string; height: string; class: string; tip: string }[];
    grid: { id: string; y: string; labelY: string; text: string }[];
    ticks: { id: string; x: string; anchor: string; text: string }[];
  };
  hasReceipt: boolean; receiptDigest: string; receipt: Pair[]; receiptJson: string;
};
export type RecordRow = { id: string; version: string; committedAt: string; commitSequence: string; cells: Cell[] };
export type RecordsCard = {
  id: string; title: string; summary: string; collection: string; hasRows: boolean; hasNoRows: boolean;
  columns: Cell[]; rows: RecordRow[];
};
export type SharedPart = { id: string; title: string; class: string; text: string; isNote: boolean; isForm: boolean; message: string; views: View[]; recordComponents: RecordComponent[]; hasRecordComponents: boolean };
export type SharedPageProps = {
  presentation: PresentationProps;
  title: string; shareId: string; status: string; ownerLabel: string; expiresAt: string;
  canAddToCanvas: boolean; addedToCanvas: boolean; available: boolean; message: string;
  needsSignIn: boolean; canSignUp: boolean; canAccept: boolean; signInAction: string; signInCsrf: string;
  canSignOut: boolean; csrfSignout: string;
  parts: SharedPart[]; views: View[]; recordCards: RecordsCard[]; recordsTruncated: boolean;
  sampleData: boolean;
};
export type SharedApp = {
  presentation: PresentationProps;
  id: string; title: string; href: string; ownerLabel: string; canAddToCanvas: boolean;
  parts: SharedPart[]; views: View[]; recordCards: RecordsCard[]; recordsTruncated: boolean;
};
export type SharedCanvasProps = { accountLabel: string; csrfSignout: string; canSignOut: boolean; hasWorkspace: boolean; hasApps: boolean; apps: SharedApp[] };
