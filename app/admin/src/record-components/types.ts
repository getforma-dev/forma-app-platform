/** Closed server projection props: all arithmetic and display values are authorized server results. */
export type RecordFilter = { id: string; label: string; isText: boolean; isEnum: boolean; isDate: boolean; value: string; start: string; end: string; options: { value: string; label: string; selected: boolean }[] };
export type RecordComponent = {
  cardId: string; id: string; title: string; kind: string; state: string; message: string; schema: string;
  ready: boolean; empty: boolean; failed: boolean; hasSummary: boolean; summary: string; summaryRaw: string;
  hasTable: boolean; columns: { id: string; label: string }[];
  rows: { id: string; version: string; cells: { id: string; text: string; raw: string }[] }[];
  hasGroups: boolean; groups: { id: string; label: string; value: string; raw: string; width: string }[];
  population: string; truncated: boolean; hasBudget: boolean; budget: string; remaining: string; ratio: string;
  filters: RecordFilter[]; hasFilters: boolean; filterEpoch: number;
  hasReceipt: boolean; receiptDigest: string; receiptJson: string;
};
export type WireRecordFilter = Omit<RecordFilter,'isText'|'isEnum'|'isDate'> & { is_text:boolean;is_enum:boolean;is_date:boolean };
export type WireRecordComponent = Omit<RecordComponent,'cardId'|'hasSummary'|'summaryRaw'|'hasTable'|'hasGroups'|'hasBudget'|'hasFilters'|'filterEpoch'|'hasReceipt'|'receiptDigest'|'receiptJson'|'filters'> & {
  card_id:string;has_summary:boolean;summary_raw:string;has_table:boolean;has_groups:boolean;has_budget:boolean;has_filters:boolean;filter_epoch:number;
  has_receipt:boolean;receipt_digest:string;receipt_json:string;filters:WireRecordFilter[];
};
