//! First-party, records-only starting definitions. Selecting a template reads
//! this catalog; only confirming compose admits and publishes a new saved work.
//! No source data, records, permissions or model output are part of the catalog.
use super::{plan, Origin};
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug)]
pub struct Template {
    pub id: &'static str,
    pub title: &'static str,
    pub industry: &'static str,
    pub category: &'static str,
    pub description: &'static str,
}

pub const CATALOG: &[Template] = &[
    Template {
        id: "equipment-requests",
        title: "Equipment requests",
        industry: "Operations",
        category: "Trackers",
        description: "Keep equipment requests with an item, requester, and notes.",
    },
    Template {
        id: "reading-list",
        title: "Reading list",
        industry: "Education",
        category: "Lists",
        description: "Keep a reading list with a title, an author, and notes.",
    },
    Template {
        id: "expense-log",
        title: "Expense log",
        industry: "Finance",
        category: "Logs",
        description: "Record expenses with a description, amount in USD, and date.",
    },
    Template {
        id: "kept-list",
        title: "Team list",
        industry: "General",
        category: "Lists",
        description: "Keep a team list with a name, owner, and notes.",
    },
];

pub fn find(id: &str) -> Option<&'static Template> {
    CATALOG.iter().find(|template| template.id == id)
}

impl Template {
    pub fn metadata(&self) -> Value {
        json!({"id": self.id, "title": self.title, "industry": self.industry,
            "category": self.category, "description": self.description})
    }

    pub fn content(&self) -> Value {
        let (schema, input, fields, writable) = match self.id {
            "equipment-requests" => (
                "request",
                "requests",
                json!([
                    {"id":"item","value_type":{"type":"text"},"nullable":false},
                    {"id":"requester","value_type":{"type":"text"},"nullable":false},
                    {"id":"notes","value_type":{"type":"text"},"nullable":true}
                ]),
                json!(["item", "requester", "notes"]),
            ),
            "reading-list" => (
                "book",
                "books",
                json!([
                    {"id":"title","value_type":{"type":"text"},"nullable":false},
                    {"id":"author","value_type":{"type":"text"},"nullable":false},
                    {"id":"notes","value_type":{"type":"text"},"nullable":true}
                ]),
                json!(["title", "author", "notes"]),
            ),
            "expense-log" => (
                "expense",
                "expenses",
                json!([
                    {"id":"description","value_type":{"type":"text"},"nullable":false},
                    {"id":"amount","value_type":{"type":"money","currency":"USD","minor_unit_exponent":2},"nullable":false},
                    {"id":"date","value_type":{"type":"date"},"nullable":false}
                ]),
                json!(["description", "amount", "date"]),
            ),
            _ => (
                "item",
                "items",
                json!([
                    {"id":"name","value_type":{"type":"text"},"nullable":false},
                    {"id":"owner","value_type":{"type":"text"},"nullable":false},
                    {"id":"notes","value_type":{"type":"text"},"nullable":true}
                ]),
                json!(["name", "owner", "notes"]),
            ),
        };
        json!({
            "title": self.title, "description": self.description,
            "inputs": [{"id":input,"contract_id":format!("{schema}_v1"),"contract_version":1,"kind":"records","fields":fields}],
            "views": [
                {"id":"add","kind":"form","renderer_version":1,"title":format!("Add {schema}"),"command_id":"add"},
                {"id":"guide","kind":"note","renderer_version":1,"title":"Getting started","text":"Add a record using the form. Saved records carry receipts. Share this app to let a colleague follow its live updates."}
            ],
            "record_schemas": [{"id":schema,"version":1,"fields":fields}],
            "commands": [{"id":"add","record_schema_id":schema,"operation":"create","writable_fields":writable,"transitions":[]}],
            "links": [],
            "default_layout": [
                {"view_id":"guide","column":0,"row":0,"width":12},
                {"view_id":"add","column":0,"row":1,"width":12}
            ]
        })
    }

    /// Admit the exact starting definition under the caller's trusted scope.
    /// An unchanged description needs no model; this is real instantiation,
    /// with empty collections and ordinary publication/command authority.
    pub fn instantiate(
        &self,
        organization: &str,
        app: &str,
    ) -> Result<plan::Planned, plan::Refusal> {
        self.plan(self.content(), organization, app)
    }

    /// Re-admit a model-customized template after the same authoring checks.
    pub fn plan(
        &self,
        mut content: Value,
        organization: &str,
        app: &str,
    ) -> Result<plan::Planned, plan::Refusal> {
        let description = content
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .strip_prefix(plan::GENERATED_PREFIX)
            .unwrap_or_else(|| {
                content
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            });
        content["description"] = json!(format!(
            "{}{} template\n{}",
            super::transform::ADAPTED_PREFIX,
            self.title,
            description
        ));
        let admitted = plan::admit(&content, organization, app)?;
        plan::require_runnable(&content)?;
        plan::require_authored(&content)?;
        plan::require_entry(&content)?;
        Ok(plan::Planned {
            title: content["title"].as_str().unwrap_or(self.title).to_owned(),
            admitted,
            origin: Origin::Adapted,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_template_is_admitted_without_seed_data_or_source_authority() {
        for template in CATALOG {
            let planned = template
                .instantiate(
                    "00000000-0000-4000-8000-000000000001",
                    "00000000-0000-4000-8000-000000000011",
                )
                .unwrap();
            let content = crate::admission::decode(&planned.admitted.canonical).unwrap();
            assert_eq!(planned.origin, Origin::Adapted);
            assert!(content["inputs"]
                .as_array()
                .unwrap()
                .iter()
                .all(|input| input["kind"] == "records"));
            assert_eq!(content["default_layout"][0]["view_id"], "guide");
            assert!(!content.to_string().contains("seed"));
            assert!(find(template.id).is_some());
        }
        assert!(find("unknown").is_none());
    }
}
