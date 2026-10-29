//! Whole-output admission of one `app_definition` of contract edition
//! impl-2026-10-28.1: `$defs.app_definition` in `contracts/schema.py` and
//! `validate`/`_definition` in `contracts/validate.py` (design section 6.2,
//! C04, C13).
//!
//! The document is decoded as forma-json-v1 and walked against the closed
//! grammar, the renderer and operator registries and the complexity limits;
//! then its internal references are checked in validate.py's order and with its
//! messages, and the submitted digest must equal the content's. Admission fails
//! whole or yields one immutable [`AdmittedDefinition`]; nothing partial
//! survives. A document breaking several grammar rules reports its most severe
//! class (grant, executable content, unknown field, renderer, operator, view
//! count, operation count, size, encoding), the first met within that class.
//!
//! Free text (titles, descriptions, note text and text literals) is data that
//! trusted renderers escape; it is bounded but never searched. Everything the
//! grammar refuses (undeclared keys and their values, mistyped members, strings
//! failing their identifier, enum or format grammar) is searched for authority
//! claims and script, markup, SQL, URL or path syntax, which only choose the
//! reported reason. The envelope's organization, app and revision identifiers
//! are checked for syntax only and confer nothing: storage binds admitted
//! content to the trusted request's scope (C01, ADR-002).
use super::{canonical, digest, AdmissionError};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::RangeInclusive;

/// Views in one definition.
pub const MAX_VIEWS: usize = 24;
/// Operations in one view's query.
pub const MAX_OPERATIONS: usize = 8;
/// Canonical bytes of one definition's content (256 KiB).
pub const MAX_DEFINITION_BYTES: usize = 262_144;
/// Rows on one result page.
pub const MAX_PAGE_ROWS: u64 = 500;
/// The registered definition schema version.
pub const SCHEMA_VERSION: u32 = 1;
/// The registered renderer version.
pub const RENDERER_VERSION: u32 = 1;
/// The registered operator version.
pub const OPERATOR_VERSION: u32 = 1;
/// Registered renderer kinds (C04).
pub const RENDERERS: [&str; 9] = [
    "summary", "table", "detail", "timeline", "chart", "filter", "form", "note", "status",
];
/// Registered query operators (C04).
#[rustfmt::skip]
pub const OPERATORS: [&str; 8] = [
    "project", "filter", "aggregate", "sort", "limit", "time_bucket", "ratio", "lookup",
];
const MEASURES: [&str; 4] = ["count", "sum", "min", "max"];
const COMPARISONS: [&str; 6] = ["eq", "ne", "lt", "lte", "gt", "gte"];
const LOOKUP_RULES: [&str; 2] = ["exact", "cidr_exact_then_longest"];
#[rustfmt::skip]
const VALUE_TYPES: [&str; 9] = [
    "text", "boolean", "integer", "decimal", "date", "timestamp", "enum", "reference", "money",
];
#[rustfmt::skip]
const LITERALS: [&str; 10] = [
    "text", "boolean", "integer", "decimal", "date", "timestamp", "enum", "reference", "money",
    "null",
];
const BUCKET_SECONDS: [u64; 4] = [60, 300, 3600, 86_400];
/// Free text bound in Unicode scalar values (schema `maxLength`).
const TEXT_CHARACTERS: usize = 2000;
/// Undeclared key words claiming authority. Actor and tenant authority come
/// only from authenticated context and verified membership (C01).
#[rustfmt::skip]
const AUTHORITY_WORDS: [&str; 35] = [
    "access", "acl", "acls", "admin", "admins", "approver", "authority", "capabilities",
    "capability", "entitlement", "entitlements", "grant", "grants", "member", "members",
    "membership", "memberships", "organization", "organizations", "owner", "owners",
    "permission", "permissions", "policies", "policy", "principal", "principals", "privilege",
    "privileges", "role", "roles", "scope", "scopes", "tenant", "tenants",
];
/// Undeclared key words naming code, markup, queries or locators (C04).
#[rustfmt::skip]
const EXECUTABLE_WORDS: [&str; 41] = [
    "callback", "callbacks", "cmd", "css", "endpoint", "endpoints", "eval", "exec", "expression",
    "expressions", "file", "files", "fmir", "formula", "formulas", "function", "functions",
    "handler", "handlers", "href", "html", "javascript", "js", "jsx", "macro", "macros", "markup",
    "path", "paths", "regex", "script", "scripts", "shell", "sql", "src", "tsx", "typescript",
    "uri", "uris", "url", "urls",
];

/// One admitted, immutable definition.
#[derive(Clone, Eq, PartialEq)]
pub struct AdmittedDefinition {
    /// `sha256:` identity of [`canonical`](Self::canonical) in the
    /// `forma:app-definition:v1` domain.
    pub digest: String,
    /// forma-json-v1 bytes of the definition content, at most
    /// [`MAX_DEFINITION_BYTES`].
    pub canonical: Vec<u8>,
    /// The contract schema version the definition was admitted under.
    pub schema_version: u32,
    /// The registered renderer version every view names.
    pub renderer_version: u32,
    /// The registered operator version every query names.
    pub operator_version: u32,
    /// Views in the definition, at most [`MAX_VIEWS`].
    pub view_count: usize,
}

impl fmt::Debug for AdmittedDefinition {
    // Content can carry organization data; only its size is rendered.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AdmittedDefinition")
            .field("digest", &self.digest)
            .field("canonical_bytes", &self.canonical.len())
            .field("schema_version", &self.schema_version)
            .field("renderer_version", &self.renderer_version)
            .field("operator_version", &self.operator_version)
            .field("view_count", &self.view_count)
            .finish()
    }
}

/// Admits one definition document whole, or refuses it with a coded reason.
pub fn admit_definition(bytes: &[u8]) -> Result<AdmittedDefinition, AdmissionError> {
    let document = canonical::decode(bytes)?;
    let mut walk = Walk::default();
    let canonical = match document.get("content") {
        Some(content) => Some(canonical::encode(content)?),
        None => None,
    };
    if let Some(size) = canonical
        .as_ref()
        .map(Vec::len)
        .filter(|size| *size > MAX_DEFINITION_BYTES)
    {
        walk.fail(AdmissionError::TooLarge(size));
    }
    let definition = walk.definition(&document);
    if let Some(error) = walk.worst {
        return Err(error);
    }
    let (Some(definition), Some(canonical)) = (definition, canonical) else {
        return Err(refused("$", "incomplete definition"));
    };
    definition.content.references()?;
    let identity = digest::digest(digest::APP_DEFINITION, &canonical);
    if identity != definition.digest {
        return Err(refused("$.digest", "definition digest mismatch"));
    }
    Ok(AdmittedDefinition {
        digest: identity,
        canonical,
        schema_version: SCHEMA_VERSION,
        renderer_version: RENDERER_VERSION,
        operator_version: OPERATOR_VERSION,
        view_count: definition.content.views.len(),
    })
}

/// The parts of a well-formed definition its reference rules read.
struct Definition {
    content: Content,
    digest: String,
}

struct Content {
    inputs: Vec<Contract>,
    views: Vec<View>,
    record_schemas: Vec<Contract>,
    commands: Vec<Command>,
    links: Vec<Link>,
    layout: Vec<Position>,
}

/// An input contract or a record schema: an id and its typed fields.
struct Contract {
    id: String,
    fields: Vec<Field>,
}

struct Field {
    id: String,
    value_type: ValueType,
    nullable: bool,
}

/// Money admits only USD at exponent 2, so every money type is equal.
#[derive(PartialEq)]
enum ValueType {
    Text,
    Boolean,
    Integer { unit: String },
    Decimal { unit: String, scale: u64 },
    Date,
    Timestamp,
    Enum(Vec<String>),
    Reference(String),
    Money,
}

struct View {
    id: String,
    kind: String,
    query: Option<Query>,
    command_id: Option<String>,
    text: bool,
}

struct Query {
    inputs: Vec<String>,
    window: Option<(String, String)>,
    operations: Vec<Operation>,
}

struct FieldRef {
    input: String,
    field: String,
}

struct Measure {
    id: String,
    sum: bool,
    field: FieldRef,
}

enum Operation {
    Project(Vec<FieldRef>),
    Filter(FieldRef, Literal),
    Aggregate {
        group_by: Vec<FieldRef>,
        measures: Vec<Measure>,
    },
    Sort {
        keys: Vec<FieldRef>,
        tiebreak: FieldRef,
    },
    Limit,
    TimeBucket(FieldRef),
    Ratio {
        numerator: String,
        /// A prior measure, or `None` for a constant.
        denominator: Option<String>,
        output: String,
    },
    Lookup {
        left: FieldRef,
        right: FieldRef,
        cidr: bool,
    },
}

enum Literal {
    Null,
    Text,
    Boolean,
    Integer { unit: String },
    Decimal { unit: String, scale: u64 },
    Date,
    Timestamp,
    Enum(String),
    Reference(String),
    Money,
}

struct Command {
    id: String,
    record_schema_id: String,
    writable: Vec<String>,
    transitions: Vec<Transition>,
}

struct Transition {
    field_id: String,
    from: String,
    to: String,
}

struct Link {
    from_view: String,
    to_view: String,
    input_id: String,
}

struct Position {
    view_id: String,
    column: u64,
    width: u64,
}

type Unregistered = fn(String) -> AdmissionError;

/// The grammar walk. Every failure is recorded, the most severe kept; a builder
/// returns `None` only after recording why, and its output is read only when
/// nothing was recorded.
#[derive(Default)]
struct Walk {
    worst: Option<AdmissionError>,
}

impl Walk {
    fn fail(&mut self, error: AdmissionError) {
        if self
            .worst
            .as_ref()
            .is_none_or(|worst| error.severity() < worst.severity())
        {
            self.worst = Some(error);
        }
    }

    /// A closed object: undeclared keys are classified, missing required ones
    /// reported.
    fn object<'v>(
        &mut self,
        path: &str,
        value: &'v Value,
        required: &[&str],
        optional: &[&str],
    ) -> Option<&'v Map<String, Value>> {
        let Some(map) = value.as_object() else {
            self.mistyped(path, value, refused(path, "expected an object"));
            return None;
        };
        for (key, child) in sorted(map) {
            if !required.contains(&key) && !optional.contains(&key) {
                self.undeclared(&member(path, key), key, child);
            }
        }
        for key in required {
            if !map.contains_key(*key) {
                self.fail(refused(&member(path, key), "required field missing"));
            }
        }
        Some(map)
    }

    /// A key outside the closed field set, classified by what it claims, plus
    /// any authority or executable content nested in its value.
    fn undeclared(&mut self, path: &str, key: &str, value: &Value) {
        self.fail(if names_authority(key) {
            AdmissionError::ClientGrant(path.to_owned())
        } else if names_executable(key) {
            AdmissionError::ExecutableContent(path.to_owned())
        } else {
            AdmissionError::UnknownField(path.to_owned())
        });
        self.scan(path, value);
    }

    /// A declared member whose type or value the contract refuses. It is no
    /// longer free text, so it is searched like undeclared content and any code
    /// or authority hidden in it is still named.
    fn mistyped(&mut self, path: &str, value: &Value, error: AdmissionError) {
        self.fail(error);
        self.scan(path, value);
    }

    /// Authority keys and executable strings anywhere inside content that the
    /// grammar did not admit.
    fn scan(&mut self, path: &str, value: &Value) {
        match value {
            Value::String(text) if executable_shape(text) => {
                self.fail(AdmissionError::ExecutableContent(path.to_owned()));
            }
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    self.scan(&element(path, index), item);
                }
            }
            Value::Object(map) => {
                for (key, child) in sorted(map) {
                    let path = member(path, key);
                    if names_authority(key) {
                        self.fail(AdmissionError::ClientGrant(path.clone()));
                    } else if names_executable(key) {
                        self.fail(AdmissionError::ExecutableContent(path.clone()));
                    }
                    self.scan(&path, child);
                }
            }
            _ => {}
        }
    }

    /// The `type` tag of a tagged union. An unknown tag leaves the variant's
    /// field set unknown, so its whole object is searched instead.
    fn tag(
        &mut self,
        path: &str,
        value: &Value,
        tags: &[&'static str],
        unregistered: Option<Unregistered>,
    ) -> Option<&'static str> {
        let Some(map) = value.as_object() else {
            self.mistyped(path, value, refused(path, "expected an object"));
            return None;
        };
        let tag = map.get("type");
        if let Some(known) = tag
            .and_then(Value::as_str)
            .and_then(|tag| tags.iter().copied().find(|known| *known == tag))
        {
            return Some(known);
        }
        let tag_path = member(path, "type");
        self.fail(match (tag, unregistered) {
            (None, _) => refused(&tag_path, "required field missing"),
            (Some(_), Some(make)) => make(tag_path),
            (Some(_), None) => refused(&tag_path, "unregistered type"),
        });
        self.scan(path, value);
        None
    }

    /// A string that must match `grammar`.
    fn string(&mut self, path: &str, value: Option<&Value>, grammar: Grammar) -> Option<String> {
        let value = value?;
        match value.as_str() {
            Some(text) if grammar.admits(text) => Some(text.to_owned()),
            _ => {
                self.mistyped(path, value, refused(path, grammar.rule()));
                None
            }
        }
    }

    fn id(&mut self, path: &str, value: Option<&Value>) -> Option<String> {
        self.string(path, value, Grammar::Id)
    }

    fn id_item(&mut self, path: &str, value: &Value) -> Option<String> {
        self.id(path, Some(value))
    }

    /// Free text: bounded but never searched, since it is data.
    fn text(&mut self, path: &str, value: Option<&Value>) {
        const RULE: &str = "expected text of at most 2000 characters";
        let Some(value) = value else {
            return;
        };
        match value.as_str() {
            Some(text) if text.chars().count() <= TEXT_CHARACTERS => {}
            Some(_) => self.fail(refused(path, RULE)),
            None => self.mistyped(path, value, refused(path, RULE)),
        }
    }

    /// One of `allowed`. Anything else is unregistered for a renderer or
    /// operator identity and an encoding failure otherwise.
    fn choice(
        &mut self,
        path: &str,
        value: Option<&Value>,
        allowed: &[&str],
        unregistered: Option<Unregistered>,
    ) -> Option<String> {
        let value = value?;
        if let Some(text) = value.as_str().filter(|text| allowed.contains(text)) {
            return Some(text.to_owned());
        }
        let error = match unregistered {
            Some(make) => make(path.to_owned()),
            None => refused(path, &format!("expected one of {}", allowed.join(", "))),
        };
        self.mistyped(path, value, error);
        None
    }

    fn version(
        &mut self,
        path: &str,
        value: Option<&Value>,
        registered: u32,
        unregistered: Option<Unregistered>,
    ) {
        let Some(value) = value else {
            return;
        };
        if value.as_u64() != Some(u64::from(registered)) {
            let error = match unregistered {
                Some(make) => make(path.to_owned()),
                None => refused(path, "unsupported version"),
            };
            self.mistyped(path, value, error);
        }
    }

    fn integer(
        &mut self,
        path: &str,
        value: Option<&Value>,
        range: RangeInclusive<u64>,
    ) -> Option<u64> {
        let value = value?;
        match value.as_u64() {
            Some(number) if range.contains(&number) => Some(number),
            _ => {
                let rule = format!(
                    "expected an integer from {} to {}",
                    range.start(),
                    range.end()
                );
                self.mistyped(path, value, refused(path, &rule));
                None
            }
        }
    }

    fn exactly(&mut self, path: &str, value: Option<&Value>, expected: u64) {
        if let Some(value) = value.filter(|value| value.as_u64() != Some(expected)) {
            self.mistyped(path, value, refused(path, &format!("expected {expected}")));
        }
    }

    fn boolean(&mut self, path: &str, value: Option<&Value>) -> Option<bool> {
        let value = value?;
        let flag = value.as_bool();
        if flag.is_none() {
            self.mistyped(path, value, refused(path, "expected true or false"));
        }
        flag
    }

    fn array<'v>(
        &mut self,
        path: &str,
        value: Option<&'v Value>,
        min: usize,
        max: usize,
    ) -> Option<&'v [Value]> {
        let value = value?;
        let Some(items) = value.as_array() else {
            self.mistyped(path, value, refused(path, "expected an array"));
            return None;
        };
        if items.len() < min {
            self.fail(refused(path, &format!("expected at least {min} items")));
        }
        if items.len() > max {
            self.fail(refused(path, &format!("expected at most {max} items")));
        }
        Some(items)
    }

    /// Every element through `walk`; all are visited even after a failure.
    fn each<T>(
        &mut self,
        path: &str,
        value: Option<&Value>,
        bounds: RangeInclusive<usize>,
        walk: fn(&mut Self, &str, &Value) -> Option<T>,
    ) -> Option<Vec<T>> {
        let items = self.array(path, value, *bounds.start(), *bounds.end())?;
        let built: Vec<Option<T>> = items
            .iter()
            .enumerate()
            .map(|(index, item)| walk(self, &element(path, index), item))
            .collect();
        built.into_iter().collect()
    }

    /// Distinct identifiers (schema `uniqueItems`).
    fn ids(
        &mut self,
        path: &str,
        value: Option<&Value>,
        bounds: RangeInclusive<usize>,
    ) -> Option<Vec<String>> {
        let ids = self.each(path, value, bounds, Self::id_item)?;
        if ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
            self.fail(refused(path, "expected unique items"));
        }
        Some(ids)
    }

    fn child<T>(
        &mut self,
        path: &str,
        value: Option<&Value>,
        walk: fn(&mut Self, &str, &Value) -> Option<T>,
    ) -> Option<T> {
        walk(self, path, value?)
    }

    fn definition(&mut self, value: &Value) -> Option<Definition> {
        let required = [
            "kind",
            "schema_version",
            "organization_id",
            "app_id",
            "revision_id",
            "content",
            "digest",
        ];
        let map = self.object("$", value, &required, &[])?;
        self.choice("$.kind", map.get("kind"), &["app_definition"], None);
        self.version(
            "$.schema_version",
            map.get("schema_version"),
            SCHEMA_VERSION,
            None,
        );
        for key in ["organization_id", "app_id", "revision_id"] {
            self.string(&member("$", key), map.get(key), Grammar::Uuid);
        }
        let content = self.child("$.content", map.get("content"), Self::content);
        let digest = self.string("$.digest", map.get("digest"), Grammar::Digest);
        Some(Definition {
            content: content?,
            digest: digest?,
        })
    }

    fn content(&mut self, path: &str, value: &Value) -> Option<Content> {
        let required = [
            "title",
            "description",
            "inputs",
            "views",
            "record_schemas",
            "commands",
            "links",
            "default_layout",
        ];
        let map = self.object(path, value, &required, &[])?;
        let at = |key: &str| member(path, key);
        self.text(&at("title"), map.get("title"));
        self.text(&at("description"), map.get("description"));
        let inputs = self.each(
            &at("inputs"),
            map.get("inputs"),
            0..=8,
            Self::input_contract,
        );
        if let Some(count) = map
            .get("views")
            .and_then(Value::as_array)
            .map(Vec::len)
            .filter(|count| *count > MAX_VIEWS)
        {
            self.fail(AdmissionError::TooManyViews(count));
        }
        let views = self.each(&at("views"), map.get("views"), 1..=usize::MAX, Self::view);
        let record_schemas = self.each(
            &at("record_schemas"),
            map.get("record_schemas"),
            0..=8,
            Self::record_schema,
        );
        let commands = self.each(&at("commands"), map.get("commands"), 0..=16, Self::command);
        let links = self.each(&at("links"), map.get("links"), 0..=48, Self::link);
        let layout = self.each(
            &at("default_layout"),
            map.get("default_layout"),
            1..=MAX_VIEWS,
            Self::position,
        );
        Some(Content {
            inputs: inputs?,
            views: views?,
            record_schemas: record_schemas?,
            commands: commands?,
            links: links?,
            layout: layout?,
        })
    }

    fn input_contract(&mut self, path: &str, value: &Value) -> Option<Contract> {
        let required = ["id", "contract_id", "contract_version", "kind", "fields"];
        let map = self.object(path, value, &required, &[])?;
        let at = |key: &str| member(path, key);
        let id = self.id(&at("id"), map.get("id"));
        self.id(&at("contract_id"), map.get("contract_id"));
        self.version(
            &at("contract_version"),
            map.get("contract_version"),
            1,
            None,
        );
        self.choice(&at("kind"), map.get("kind"), &["source", "records"], None);
        let fields = self.each(&at("fields"), map.get("fields"), 1..=32, Self::field);
        Some(Contract {
            id: id?,
            fields: fields?,
        })
    }

    fn record_schema(&mut self, path: &str, value: &Value) -> Option<Contract> {
        let map = self.object(path, value, &["id", "version", "fields"], &[])?;
        let at = |key: &str| member(path, key);
        let id = self.id(&at("id"), map.get("id"));
        self.version(&at("version"), map.get("version"), 1, None);
        let fields = self.each(&at("fields"), map.get("fields"), 1..=32, Self::field);
        Some(Contract {
            id: id?,
            fields: fields?,
        })
    }

    fn field(&mut self, path: &str, value: &Value) -> Option<Field> {
        let map = self.object(path, value, &["id", "value_type", "nullable"], &[])?;
        let at = |key: &str| member(path, key);
        let id = self.id(&at("id"), map.get("id"));
        let value_type = self.child(&at("value_type"), map.get("value_type"), Self::value_type);
        let nullable = self.boolean(&at("nullable"), map.get("nullable"));
        Some(Field {
            id: id?,
            value_type: value_type?,
            nullable: nullable?,
        })
    }

    fn value_type(&mut self, path: &str, value: &Value) -> Option<ValueType> {
        let tag = self.tag(path, value, &VALUE_TYPES, None)?;
        let at = |key: &str| member(path, key);
        Some(match tag {
            "integer" => {
                let map = self.object(path, value, &["type", "unit"], &[])?;
                ValueType::Integer {
                    unit: self.id(&at("unit"), map.get("unit"))?,
                }
            }
            "decimal" => {
                let map = self.object(path, value, &["type", "unit", "scale"], &[])?;
                let unit = self.id(&at("unit"), map.get("unit"));
                let scale = self.integer(&at("scale"), map.get("scale"), 0..=18);
                ValueType::Decimal {
                    unit: unit?,
                    scale: scale?,
                }
            }
            "enum" => {
                let map = self.object(path, value, &["type", "values"], &[])?;
                ValueType::Enum(self.ids(&at("values"), map.get("values"), 1..=32)?)
            }
            "reference" => {
                let map = self.object(path, value, &["type", "target_input"], &[])?;
                ValueType::Reference(self.id(&at("target_input"), map.get("target_input"))?)
            }
            "money" => {
                let required = ["type", "currency", "minor_unit_exponent"];
                let map = self.object(path, value, &required, &[])?;
                self.choice(&at("currency"), map.get("currency"), &["USD"], None);
                self.exactly(
                    &at("minor_unit_exponent"),
                    map.get("minor_unit_exponent"),
                    2,
                );
                ValueType::Money
            }
            plain => {
                self.object(path, value, &["type"], &[])?;
                match plain {
                    "text" => ValueType::Text,
                    "boolean" => ValueType::Boolean,
                    "date" => ValueType::Date,
                    _ => ValueType::Timestamp,
                }
            }
        })
    }

    fn literal(&mut self, path: &str, value: &Value) -> Option<Literal> {
        let tag = self.tag(path, value, &LITERALS, None)?;
        let at = |key: &str| member(path, key);
        Some(match tag {
            "text" => {
                let map = self.object(path, value, &["type", "value"], &[])?;
                self.text(&at("value"), map.get("value"));
                Literal::Text
            }
            "boolean" => {
                let map = self.object(path, value, &["type", "value"], &[])?;
                self.boolean(&at("value"), map.get("value"))?;
                Literal::Boolean
            }
            "integer" => {
                let map = self.object(path, value, &["type", "value", "unit"], &[])?;
                self.string(&at("value"), map.get("value"), Grammar::Integer);
                Literal::Integer {
                    unit: self.id(&at("unit"), map.get("unit"))?,
                }
            }
            "decimal" => {
                let required = ["type", "coefficient", "scale", "unit"];
                let map = self.object(path, value, &required, &[])?;
                self.string(&at("coefficient"), map.get("coefficient"), Grammar::Integer);
                let scale = self.integer(&at("scale"), map.get("scale"), 0..=18);
                let unit = self.id(&at("unit"), map.get("unit"));
                Literal::Decimal {
                    unit: unit?,
                    scale: scale?,
                }
            }
            "date" => {
                let map = self.object(path, value, &["type", "value"], &[])?;
                self.string(&at("value"), map.get("value"), Grammar::Date)?;
                Literal::Date
            }
            "timestamp" => {
                let map = self.object(path, value, &["type", "value"], &[])?;
                self.string(&at("value"), map.get("value"), Grammar::Time)?;
                Literal::Timestamp
            }
            "enum" => {
                let map = self.object(path, value, &["type", "value"], &[])?;
                Literal::Enum(self.id(&at("value"), map.get("value"))?)
            }
            "reference" => {
                let map = self.object(path, value, &["type", "input_id", "record_id"], &[])?;
                let input = self.id(&at("input_id"), map.get("input_id"));
                self.string(&at("record_id"), map.get("record_id"), Grammar::Uuid);
                Literal::Reference(input?)
            }
            "money" => {
                let required = ["type", "minor_units", "currency", "minor_unit_exponent"];
                let map = self.object(path, value, &required, &[])?;
                self.string(&at("minor_units"), map.get("minor_units"), Grammar::Integer);
                self.choice(&at("currency"), map.get("currency"), &["USD"], None);
                self.exactly(
                    &at("minor_unit_exponent"),
                    map.get("minor_unit_exponent"),
                    2,
                );
                Literal::Money
            }
            _ => {
                self.object(path, value, &["type"], &[])?;
                Literal::Null
            }
        })
    }

    fn view(&mut self, path: &str, value: &Value) -> Option<View> {
        let required = ["id", "kind", "renderer_version", "title"];
        let map = self.object(path, value, &required, &["query", "command_id", "text"])?;
        let at = |key: &str| member(path, key);
        let id = self.id(&at("id"), map.get("id"));
        let kind = self.choice(
            &at("kind"),
            map.get("kind"),
            &RENDERERS,
            Some(AdmissionError::UnregisteredRenderer),
        );
        self.version(
            &at("renderer_version"),
            map.get("renderer_version"),
            RENDERER_VERSION,
            Some(AdmissionError::UnregisteredRenderer),
        );
        self.text(&at("title"), map.get("title"));
        let query = map
            .get("query")
            .map(|query| self.query(&at("query"), query));
        let command_id = map
            .get("command_id")
            .map(|command| self.id(&at("command_id"), Some(command)));
        self.text(&at("text"), map.get("text"));
        Some(View {
            id: id?,
            kind: kind?,
            query: present(query)?,
            command_id: present(command_id)?,
            text: map.contains_key("text"),
        })
    }

    fn query(&mut self, path: &str, value: &Value) -> Option<Query> {
        let required = ["operator_version", "inputs", "operations", "null_policy"];
        let map = self.object(path, value, &required, &["window"])?;
        let at = |key: &str| member(path, key);
        self.version(
            &at("operator_version"),
            map.get("operator_version"),
            OPERATOR_VERSION,
            Some(AdmissionError::UnregisteredOperator),
        );
        let inputs = self.ids(&at("inputs"), map.get("inputs"), 1..=8);
        if let Some(count) = map
            .get("operations")
            .and_then(Value::as_array)
            .map(Vec::len)
            .filter(|count| *count > MAX_OPERATIONS)
        {
            self.fail(AdmissionError::TooManyOperations(count));
        }
        let operations = self.each(
            &at("operations"),
            map.get("operations"),
            1..=usize::MAX,
            Self::operation,
        );
        self.choice(
            &at("null_policy"),
            map.get("null_policy"),
            &["unknown"],
            None,
        );
        let window = map
            .get("window")
            .map(|window| self.window(&at("window"), window));
        Some(Query {
            inputs: inputs?,
            window: present(window)?,
            operations: operations?,
        })
    }

    fn window(&mut self, path: &str, value: &Value) -> Option<(String, String)> {
        let map = self.object(path, value, &["start", "end", "bounds"], &[])?;
        let at = |key: &str| member(path, key);
        let start = self.string(&at("start"), map.get("start"), Grammar::Time);
        let end = self.string(&at("end"), map.get("end"), Grammar::Time);
        self.choice(&at("bounds"), map.get("bounds"), &["half_open"], None);
        Some((start?, end?))
    }

    fn operation(&mut self, path: &str, value: &Value) -> Option<Operation> {
        let operator = self.tag(
            path,
            value,
            &OPERATORS,
            Some(AdmissionError::UnregisteredOperator),
        )?;
        let at = |key: &str| member(path, key);
        Some(match operator {
            "project" => {
                let map = self.object(path, value, &["type", "fields"], &[])?;
                Operation::Project(self.each(
                    &at("fields"),
                    map.get("fields"),
                    1..=32,
                    Self::field_ref,
                )?)
            }
            "filter" => {
                let required = ["type", "field", "comparison", "value"];
                let map = self.object(path, value, &required, &[])?;
                let field = self.child(&at("field"), map.get("field"), Self::field_ref);
                self.choice(
                    &at("comparison"),
                    map.get("comparison"),
                    &COMPARISONS,
                    Some(AdmissionError::UnregisteredOperator),
                );
                let literal = self.child(&at("value"), map.get("value"), Self::literal);
                Operation::Filter(field?, literal?)
            }
            "aggregate" => {
                let map = self.object(path, value, &["type", "group_by", "measures"], &[])?;
                let group_by =
                    self.each(&at("group_by"), map.get("group_by"), 0..=4, Self::field_ref);
                let measures =
                    self.each(&at("measures"), map.get("measures"), 1..=8, Self::measure);
                Operation::Aggregate {
                    group_by: group_by?,
                    measures: measures?,
                }
            }
            "sort" => {
                let map = self.object(path, value, &["type", "fields", "stable_tiebreak"], &[])?;
                let keys = self.each(&at("fields"), map.get("fields"), 1..=4, Self::sort_key);
                let tiebreak = self.child(
                    &at("stable_tiebreak"),
                    map.get("stable_tiebreak"),
                    Self::field_ref,
                );
                Operation::Sort {
                    keys: keys?,
                    tiebreak: tiebreak?,
                }
            }
            "limit" => {
                let map = self.object(path, value, &["type", "rows"], &[])?;
                if let Some(rows) = map.get("rows") {
                    match rows.as_u64() {
                        Some(1..=MAX_PAGE_ROWS) => {}
                        Some(count) if count > MAX_PAGE_ROWS => {
                            self.fail(refused(&at("rows"), "a page holds at most 500 rows"));
                        }
                        _ => {
                            let error = refused(&at("rows"), "expected an integer from 1 to 500");
                            self.mistyped(&at("rows"), rows, error);
                        }
                    }
                }
                Operation::Limit
            }
            "time_bucket" => {
                let required = ["type", "field", "seconds", "timezone", "output_field"];
                let map = self.object(path, value, &required, &[])?;
                let field = self.child(&at("field"), map.get("field"), Self::field_ref);
                if let Some(seconds) = map.get("seconds").filter(|seconds| {
                    !seconds
                        .as_u64()
                        .is_some_and(|width| BUCKET_SECONDS.contains(&width))
                }) {
                    let error = refused(&at("seconds"), "expected one of 60, 300, 3600, 86400");
                    self.mistyped(&at("seconds"), seconds, error);
                }
                self.choice(&at("timezone"), map.get("timezone"), &["UTC"], None);
                self.id(&at("output_field"), map.get("output_field"));
                Operation::TimeBucket(field?)
            }
            "ratio" => {
                let required = [
                    "type",
                    "numerator",
                    "denominator",
                    "output_field",
                    "output_unit",
                    "zero_denominator",
                    "rounding",
                ];
                let map = self.object(path, value, &required, &[])?;
                let numerator = self.id(&at("numerator"), map.get("numerator"));
                let denominator = self.child(
                    &at("denominator"),
                    map.get("denominator"),
                    Self::denominator,
                );
                let output = self.id(&at("output_field"), map.get("output_field"));
                self.id(&at("output_unit"), map.get("output_unit"));
                self.choice(
                    &at("zero_denominator"),
                    map.get("zero_denominator"),
                    &["error"],
                    None,
                );
                self.choice(
                    &at("rounding"),
                    map.get("rounding"),
                    &["exact_rational"],
                    None,
                );
                Operation::Ratio {
                    numerator: numerator?,
                    denominator: denominator?,
                    output: output?,
                }
            }
            "lookup" => {
                let required = ["type", "left", "right", "rule", "cardinality"];
                let map = self.object(path, value, &required, &[])?;
                let left = self.child(&at("left"), map.get("left"), Self::field_ref);
                let right = self.child(&at("right"), map.get("right"), Self::field_ref);
                let rule = self.choice(
                    &at("rule"),
                    map.get("rule"),
                    &LOOKUP_RULES,
                    Some(AdmissionError::UnregisteredOperator),
                );
                self.choice(
                    &at("cardinality"),
                    map.get("cardinality"),
                    &["at_most_one"],
                    None,
                );
                Operation::Lookup {
                    left: left?,
                    right: right?,
                    cidr: rule? == "cidr_exact_then_longest",
                }
            }
            _ => return None,
        })
    }

    /// A prior measure's id, or `None` for a positive integer constant.
    fn denominator(&mut self, path: &str, value: &Value) -> Option<Option<String>> {
        let tag = self.tag(path, value, &["measure", "constant"], None)?;
        let at = |key: &str| member(path, key);
        if tag == "measure" {
            let map = self.object(path, value, &["type", "id"], &[])?;
            return Some(Some(self.id(&at("id"), map.get("id"))?));
        }
        let map = self.object(path, value, &["type", "value", "unit"], &[])?;
        self.string(&at("value"), map.get("value"), Grammar::Positive);
        self.id(&at("unit"), map.get("unit"));
        Some(None)
    }

    fn field_ref(&mut self, path: &str, value: &Value) -> Option<FieldRef> {
        let map = self.object(path, value, &["input_id", "field_id"], &[])?;
        let input = self.id(&member(path, "input_id"), map.get("input_id"));
        let field = self.id(&member(path, "field_id"), map.get("field_id"));
        Some(FieldRef {
            input: input?,
            field: field?,
        })
    }

    fn sort_key(&mut self, path: &str, value: &Value) -> Option<FieldRef> {
        let map = self.object(path, value, &["field", "direction"], &[])?;
        let field = self.child(&member(path, "field"), map.get("field"), Self::field_ref);
        self.choice(
            &member(path, "direction"),
            map.get("direction"),
            &["asc", "desc"],
            None,
        );
        field
    }

    fn measure(&mut self, path: &str, value: &Value) -> Option<Measure> {
        let map = self.object(path, value, &["id", "operation", "field"], &[])?;
        let at = |key: &str| member(path, key);
        let id = self.id(&at("id"), map.get("id"));
        let operation = self.choice(
            &at("operation"),
            map.get("operation"),
            &MEASURES,
            Some(AdmissionError::UnregisteredOperator),
        );
        let field = self.child(&at("field"), map.get("field"), Self::field_ref);
        Some(Measure {
            id: id?,
            sum: operation? == "sum",
            field: field?,
        })
    }

    fn command(&mut self, path: &str, value: &Value) -> Option<Command> {
        let required = [
            "id",
            "record_schema_id",
            "operation",
            "writable_fields",
            "transitions",
        ];
        let map = self.object(path, value, &required, &[])?;
        let at = |key: &str| member(path, key);
        let id = self.id(&at("id"), map.get("id"));
        let record_schema_id = self.id(&at("record_schema_id"), map.get("record_schema_id"));
        self.choice(
            &at("operation"),
            map.get("operation"),
            &["create", "update"],
            None,
        );
        let writable = self.ids(&at("writable_fields"), map.get("writable_fields"), 1..=32);
        let transitions = self.each(
            &at("transitions"),
            map.get("transitions"),
            0..=32,
            Self::transition,
        );
        Some(Command {
            id: id?,
            record_schema_id: record_schema_id?,
            writable: writable?,
            transitions: transitions?,
        })
    }

    fn transition(&mut self, path: &str, value: &Value) -> Option<Transition> {
        let map = self.object(path, value, &["field_id", "from", "to"], &[])?;
        let at = |key: &str| member(path, key);
        let field_id = self.id(&at("field_id"), map.get("field_id"));
        let from = self.id(&at("from"), map.get("from"));
        let to = self.id(&at("to"), map.get("to"));
        Some(Transition {
            field_id: field_id?,
            from: from?,
            to: to?,
        })
    }

    fn link(&mut self, path: &str, value: &Value) -> Option<Link> {
        let map = self.object(path, value, &["from_view", "to_view", "input_id"], &[])?;
        let at = |key: &str| member(path, key);
        let from_view = self.id(&at("from_view"), map.get("from_view"));
        let to_view = self.id(&at("to_view"), map.get("to_view"));
        let input_id = self.id(&at("input_id"), map.get("input_id"));
        Some(Link {
            from_view: from_view?,
            to_view: to_view?,
            input_id: input_id?,
        })
    }

    fn position(&mut self, path: &str, value: &Value) -> Option<Position> {
        let map = self.object(path, value, &["view_id", "column", "row", "width"], &[])?;
        let at = |key: &str| member(path, key);
        let view_id = self.id(&at("view_id"), map.get("view_id"));
        let column = self.integer(&at("column"), map.get("column"), 0..=11);
        self.integer(&at("row"), map.get("row"), 0..=100);
        let width = self.integer(&at("width"), map.get("width"), 1..=12);
        Some(Position {
            view_id: view_id?,
            column: column?,
            width: width?,
        })
    }
}

type Fields<'a> = BTreeMap<&'a str, BTreeMap<&'a str, &'a Field>>;

impl Content {
    /// validate.py `_definition`: identifiers, references, declared commands,
    /// operator typing and layout, in its order and with its messages.
    fn references(&self) -> Result<(), AdmissionError> {
        let inputs = index(
            &self.inputs,
            |input| &input.id,
            "$.content.inputs",
            "input id",
        )?;
        let mut fields: Fields = BTreeMap::new();
        for (position, input) in self.inputs.iter().enumerate() {
            let path = format!("$.content.inputs[{position}].fields");
            let declared = index(&input.fields, |field| &field.id, &path, "input field")?;
            fields.insert(&input.id, declared);
        }
        index(
            &self.record_schemas,
            |schema| &schema.id,
            "$.content.record_schemas",
            "record schema",
        )?;
        let mut record_fields: Fields = BTreeMap::new();
        for (position, schema) in self.record_schemas.iter().enumerate() {
            let path = format!("$.content.record_schemas[{position}].fields");
            let declared = index(&schema.fields, |field| &field.id, &path, "record field")?;
            record_fields.insert(&schema.id, declared);
        }
        let views = index(&self.views, |view| &view.id, "$.content.views", "view id")?;
        let commands = index(
            &self.commands,
            |command| &command.id,
            "$.content.commands",
            "command id",
        )?;

        for (collection, contracts) in [
            ("inputs", &self.inputs),
            ("record_schemas", &self.record_schemas),
        ] {
            for (position, contract) in contracts.iter().enumerate() {
                for (slot, field) in contract.fields.iter().enumerate() {
                    if let ValueType::Reference(target) = &field.value_type {
                        let path =
                            format!("$.content.{collection}[{position}].fields[{slot}].value_type");
                        require(
                            inputs.contains_key(target.as_str()),
                            &path,
                            "unknown reference target",
                        )?;
                    }
                }
            }
        }

        for (position, command) in self.commands.iter().enumerate() {
            let path = format!("$.content.commands[{position}]");
            let Some(schema) = record_fields.get(command.record_schema_id.as_str()) else {
                return Err(refused(&path, "unknown record schema"));
            };
            let known = command
                .writable
                .iter()
                .all(|field| schema.contains_key(field.as_str()));
            require(known, &path, "unknown writable field")?;
            for (slot, transition) in command.transitions.iter().enumerate() {
                let path = format!("{path}.transitions[{slot}]");
                let Some(ValueType::Enum(values)) = schema
                    .get(transition.field_id.as_str())
                    .map(|field| &field.value_type)
                else {
                    return Err(refused(&path, "transition requires enum field"));
                };
                let writable = command.writable.contains(&transition.field_id);
                require(writable, &path, "transition field is not writable")?;
                let known = values.contains(&transition.from) && values.contains(&transition.to);
                require(known, &path, "unknown transition value")?;
            }
        }

        for (position, view) in self.views.iter().enumerate() {
            let path = format!("$.content.views[{position}]");
            if view.kind == "form" {
                let declared = view
                    .command_id
                    .as_deref()
                    .is_some_and(|command| commands.contains_key(command));
                let rule = "form requires declared command without query";
                require(declared && view.query.is_none(), &path, rule)?;
            } else {
                require(
                    view.command_id.is_none(),
                    &path,
                    "only a form invokes a command",
                )?;
            }
            if view.kind == "note" {
                require(
                    view.text && view.query.is_none(),
                    &path,
                    "note requires literal text",
                )?;
            } else {
                require(!view.text, &path, "literal text only belongs to notes")?;
            }
            if view.kind != "form" && view.kind != "note" {
                require(view.query.is_some(), &path, "data view requires query")?;
            }
            if let Some(query) = &view.query {
                query.references(&format!("{path}.query"), &fields)?;
            }
        }

        let layout = index(
            &self.layout,
            |position| &position.view_id,
            "$.content.default_layout",
            "layout view",
        )?;
        let complete = layout.keys().eq(views.keys());
        require(
            complete,
            "$.content.default_layout",
            "layout must name each view exactly once",
        )?;
        for (slot, position) in self.layout.iter().enumerate() {
            let path = format!("$.content.default_layout[{slot}]");
            require(
                position.column + position.width <= 12,
                &path,
                "layout exceeds grid",
            )?;
        }
        for (slot, link) in self.links.iter().enumerate() {
            let known = views.contains_key(link.from_view.as_str())
                && views.contains_key(link.to_view.as_str())
                && inputs.contains_key(link.input_id.as_str());
            require(
                known,
                &format!("$.content.links[{slot}]"),
                "unknown link target",
            )?;
        }
        Ok(())
    }
}

impl Query {
    fn references(&self, path: &str, fields: &Fields<'_>) -> Result<(), AdmissionError> {
        let known = self
            .inputs
            .iter()
            .all(|input| fields.contains_key(input.as_str()));
        require(known, path, "query names unknown input")?;
        if let Some((start, end)) = &self.window {
            // Fixed-width UTC instants order as text.
            require(
                start < end,
                &format!("{path}.window"),
                "time window must be increasing",
            )?;
        }
        let mut measures: BTreeSet<&str> = BTreeSet::new();
        for (position, operation) in self.operations.iter().enumerate() {
            let path = format!("{path}.operations[{position}]");
            let lookup = |reference: &FieldRef| self.lookup(fields, reference, &path);
            match operation {
                Operation::Project(references) => {
                    for reference in references {
                        lookup(reference)?;
                    }
                }
                Operation::Filter(reference, literal) => {
                    let compatible = compatible(literal, lookup(reference)?);
                    require(compatible, &path, "filter value type/unit mismatch")?;
                }
                Operation::Aggregate {
                    group_by,
                    measures: declared,
                } => {
                    for reference in group_by {
                        lookup(reference)?;
                    }
                    index(
                        declared,
                        |measure| &measure.id,
                        &format!("{path}.measures"),
                        "measure id",
                    )?;
                    for measure in declared {
                        let field = lookup(&measure.field)?;
                        if measure.sum {
                            let numeric = matches!(
                                field.value_type,
                                ValueType::Integer { .. }
                                    | ValueType::Decimal { .. }
                                    | ValueType::Money
                            );
                            require(numeric, &path, "sum requires numeric field")?;
                        }
                        require(
                            measures.insert(&measure.id),
                            &path,
                            "duplicate derived measure",
                        )?;
                    }
                }
                Operation::Sort { keys, tiebreak } => {
                    for reference in keys {
                        lookup(reference)?;
                    }
                    lookup(tiebreak)?;
                }
                Operation::Limit => {}
                Operation::TimeBucket(reference) => {
                    let timestamp = lookup(reference)?.value_type == ValueType::Timestamp;
                    require(timestamp, &path, "time bucket requires timestamp")?;
                }
                Operation::Lookup { left, right, cidr } => {
                    let (left, right) = (lookup(left)?, lookup(right)?);
                    require(
                        left.value_type == right.value_type,
                        &path,
                        "lookup type mismatch",
                    )?;
                    if *cidr {
                        let text = left.value_type == ValueType::Text;
                        require(text, &path, "CIDR requires text fields")?;
                    }
                }
                Operation::Ratio {
                    numerator,
                    denominator,
                    output,
                } => {
                    let declared = measures.contains(numerator.as_str());
                    require(declared, &path, "ratio requires prior declared numerator")?;
                    if let Some(denominator) = denominator {
                        let declared = measures.contains(denominator.as_str());
                        require(declared, &path, "ratio requires prior declared denominator")?;
                    }
                    require(measures.insert(output), &path, "duplicate derived measure")?;
                }
            }
        }
        Ok(())
    }

    /// validate.py `lookup`: a field of one of this query's own inputs.
    fn lookup<'f>(
        &self,
        fields: &Fields<'f>,
        reference: &FieldRef,
        path: &str,
    ) -> Result<&'f Field, AdmissionError> {
        let declared = self.inputs.contains(&reference.input);
        require(declared, path, "field uses undeclared query input")?;
        fields
            .get(reference.input.as_str())
            .and_then(|input| input.get(reference.field.as_str()))
            .copied()
            .ok_or_else(|| refused(path, "unknown query field"))
    }
}

/// validate.py `compatible`: a filter literal against the field it compares.
fn compatible(literal: &Literal, field: &Field) -> bool {
    match (literal, &field.value_type) {
        (Literal::Null, _) => field.nullable,
        (Literal::Text, ValueType::Text)
        | (Literal::Boolean, ValueType::Boolean)
        | (Literal::Date, ValueType::Date)
        | (Literal::Timestamp, ValueType::Timestamp)
        | (Literal::Money, ValueType::Money) => true,
        (Literal::Integer { unit }, ValueType::Integer { unit: declared }) => unit == declared,
        (
            Literal::Decimal { unit, scale },
            ValueType::Decimal {
                unit: declared,
                scale: places,
            },
        ) => unit == declared && scale == places,
        (Literal::Enum(value), ValueType::Enum(values)) => values.contains(value),
        (Literal::Reference(input), ValueType::Reference(target)) => input == target,
        _ => false,
    }
}

/// validate.py `unique`: items by id, refusing the first duplicate.
fn index<'a, T>(
    items: &'a [T],
    id: fn(&'a T) -> &'a String,
    path: &str,
    label: &str,
) -> Result<BTreeMap<&'a str, &'a T>, AdmissionError> {
    let mut map = BTreeMap::new();
    for (position, item) in items.iter().enumerate() {
        if map.insert(id(item).as_str(), item).is_some() {
            return Err(refused(
                &element(path, position),
                &format!("duplicate {label}"),
            ));
        }
    }
    Ok(map)
}

fn require(condition: bool, path: &str, rule: &str) -> Result<(), AdmissionError> {
    if condition {
        Ok(())
    } else {
        Err(refused(path, rule))
    }
}

fn refused(path: &str, rule: &str) -> AdmissionError {
    AdmissionError::Encoding(format!("{path}: {rule}"))
}

/// `path.key`; an over-long undeclared key is shortened in the report.
fn member(path: &str, key: &str) -> String {
    match key.get(..64) {
        Some(prefix) if key.len() > 64 => format!("{path}.{prefix}..."),
        _ => format!("{path}.{key}"),
    }
}

fn element(path: &str, index: usize) -> String {
    format!("{path}[{index}]")
}

fn sorted(map: &Map<String, Value>) -> Vec<(&str, &Value)> {
    let mut entries: Vec<(&str, &Value)> = map
        .iter()
        .map(|(key, value)| (key.as_str(), value))
        .collect();
    entries.sort_unstable_by_key(|(key, _)| *key);
    entries
}

/// An optional member: absent is `Some(None)`; present, it is `None` unless built.
fn present<T>(member: Option<Option<T>>) -> Option<Option<T>> {
    match member {
        None => Some(None),
        Some(built) => built.map(Some),
    }
}

fn names_authority(key: &str) -> bool {
    key.split('_').any(|word| AUTHORITY_WORDS.contains(&word))
}

fn names_executable(key: &str) -> bool {
    key.split('_').any(|word| EXECUTABLE_WORDS.contains(&word))
}

/// Script, markup, SQL, URL or filesystem-path syntax. Consulted only for a
/// string the grammar already refused, never for free text, so it selects the
/// reported reason and never admits anything itself.
fn executable_shape(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|word| !word.is_empty())
        .collect();
    let has = |word: &str| words.contains(&word);
    let any = |list: &[&str]| list.iter().any(|word| has(word));
    let markup = text.contains(['<', '>', '`', '{', '}']) || text.contains("=>") || calls(text);
    let sql = text.contains(';')
        || text.contains("--")
        || text.contains("/*")
        || (has("select") && any(&["from", "into"]))
        || (has("insert") && has("into"))
        || (has("delete") && has("from"))
        || (has("update") && has("set"))
        || (has("union") && has("select"))
        || (any(&["drop", "create", "alter", "truncate"])
            && any(&[
                "table", "database", "schema", "role", "user", "index", "view", "function",
            ]))
        || (any(&["grant", "revoke"]) && any(&["on", "to", "from"]));
    let scheme = ["javascript:", "vbscript:", "data:", "file:"]
        .iter()
        .any(|scheme| lower.trim_start().starts_with(scheme));
    markup || sql || scheme || text.contains("://") || filesystem_path(text)
}

/// An identifier or `$` immediately followed by `(`: a call or substitution.
fn calls(text: &str) -> bool {
    text.as_bytes().windows(2).any(|pair| {
        pair[1] == b'(' && (pair[0].is_ascii_alphanumeric() || pair[0] == b'_' || pair[0] == b'$')
    })
}

/// Absolute, home-relative, dot-relative, traversing, multi-segment or drive paths.
fn filesystem_path(text: &str) -> bool {
    text.starts_with(['/', '~'])
        || text.contains('\\')
        || text.contains("./")
        || text == ".."
        || text.matches('/').count() > 1
        || matches!(text.as_bytes(), [letter, b':', b'/', ..] if letter.is_ascii_alphabetic())
}

/// Declared string grammars of the contract (`schema.py` patterns and formats).
#[derive(Clone, Copy)]
enum Grammar {
    Id,
    Uuid,
    Digest,
    Integer,
    Positive,
    Time,
    Date,
}

impl Grammar {
    fn admits(self, text: &str) -> bool {
        match self {
            Self::Id => {
                let bytes = text.as_bytes();
                (1..=64).contains(&bytes.len())
                    && bytes[0].is_ascii_lowercase()
                    && bytes.iter().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_'
                    })
            }
            Self::Uuid => {
                let bytes = text.as_bytes();
                bytes.len() == 36
                    && bytes
                        .iter()
                        .enumerate()
                        .all(|(position, byte)| match position {
                            8 | 13 | 18 | 23 => *byte == b'-',
                            _ => lower_hex(*byte),
                        })
                    && matches!(bytes[14], b'4' | b'7')
                    && matches!(bytes[19], b'8' | b'9' | b'a' | b'b')
            }
            Self::Digest => text
                .strip_prefix("sha256:")
                .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(lower_hex)),
            Self::Integer => text == "0" || digits(text.strip_prefix('-').unwrap_or(text), 38),
            Self::Positive => digits(text, 18),
            Self::Time => {
                let bytes = text.as_bytes();
                bytes.len() == 20
                    && bytes[10] == b'T'
                    && bytes[19] == b'Z'
                    && calendar_date(&bytes[..10])
                    && clock(&bytes[11..19])
            }
            Self::Date => calendar_date(text.as_bytes()),
        }
    }

    fn rule(self) -> &'static str {
        match self {
            Self::Id => "expected an identifier [a-z][a-z0-9_]{0,63}",
            Self::Uuid => "expected a lowercase version 4 or 7 UUID",
            Self::Digest => "expected sha256: and 64 lowercase hex digits",
            Self::Integer => "expected a canonical integer of at most 38 digits",
            Self::Positive => "expected a positive integer of at most 18 digits",
            Self::Time => "expected a UTC instant YYYY-MM-DDTHH:MM:SSZ",
            Self::Date => "expected a calendar date YYYY-MM-DD",
        }
    }
}

fn lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

/// `[1-9][0-9]*` of at most `max` digits.
fn digits(text: &str, max: usize) -> bool {
    let bytes = text.as_bytes();
    (1..=max).contains(&bytes.len()) && bytes[0] != b'0' && bytes.iter().all(u8::is_ascii_digit)
}

fn number(bytes: &[u8]) -> Option<u32> {
    let mut value = 0;
    for &byte in bytes {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value * 10 + u32::from(byte - b'0');
    }
    Some(value)
}

/// `YYYY-MM-DD` naming a real day in years 1 to 9999 (Python `date.fromisoformat`).
fn calendar_date(bytes: &[u8]) -> bool {
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let (Some(year), Some(month), Some(day)) = (
        number(&bytes[..4]),
        number(&bytes[5..7]),
        number(&bytes[8..]),
    ) else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    year >= 1 && (1..=days).contains(&day)
}

/// `HH:MM:SS` within one day; leap seconds are outside this edition.
fn clock(bytes: &[u8]) -> bool {
    bytes.len() == 8
        && bytes[2] == b':'
        && bytes[5] == b':'
        && number(&bytes[..2]).is_some_and(|hour| hour <= 23)
        && number(&bytes[3..5]).is_some_and(|minute| minute <= 59)
        && number(&bytes[6..]).is_some_and(|second| second <= 59)
}
