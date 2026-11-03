//! APP-108 (ADR-011 pitch, `pivot/boxes`): boxes. A member keeps files in a
//! box (a canvas card): PDFs, images, CSV/XLSX and text. Originals are
//! immutable objects under the organization's prefix (`store`); a CSV/XLSX is
//! parsed by the sealed parser (ADR-009, `imports::sandbox`), its column
//! types inferred (`inspect`) and, once confirmed, normalized to Parquet
//! exactly as the library sources are (`register`, `app/runtime/
//! box_normalize.py`) and registered as an immutable source revision whose
//! original is the file. An app built from a box (`derive`) sends the
//! composer a capped context (file names and types, each confirmed schema
//! with at most 20 sample rows, at most 3 data files; PDFs by name only) and
//! records its provenance; its explanation card (`explain`) is model text
//! whose every number is a placeholder filled from governed results.
//!
//! "Box" is the internal term; the member-facing noun is [`NOUN`] (one
//! place; Victor chose "Folder" on 2026-10-30), the slash words are
//! [`COMMAND_ALIASES`] and the AI keyword is [`AI_KEYWORD`]
//! (`/folder <name|number> formation <instruction>`).
pub mod ask;
pub mod brief;
pub mod derive;
pub mod explain;
pub mod inspect;
pub mod intent;
pub mod register;
pub mod routes;
pub mod sniff;
pub mod store;

#[cfg(test)]
mod tests;

/// The member-facing noun for a box (singular, capitalized).
pub const NOUN: &str = "Folder";
/// The member-facing noun, lowercase.
pub const NOUN_LOWER: &str = "folder";
/// Slash command words that name the boxes command (canvas, Slack); adding
/// a word is one entry here.
pub const COMMAND_ALIASES: &[&str] = &["folder", "folders", "storage", "box", "boxes"];
/// The word after which everything is the AI instruction.
pub const AI_KEYWORD: &str = "formation";

/// At most this many bytes per file (ticket APP-108).
pub const MAX_FILE_BYTES: usize = 20 * 1024 * 1024;
/// The multipart body: one file and its framing.
pub const UPLOAD_BODY_LIMIT: usize = MAX_FILE_BYTES + 16 * 1024;
/// Data files in a derived app's model context.
pub const MAX_CONTEXT_DATA_FILES: usize = 3;
/// Sample rows of each data file in a model context and in a preview.
pub const MAX_SAMPLE_ROWS: usize = 20;
/// File names listed in a model context.
pub const MAX_CONTEXT_FILES: usize = 20;
