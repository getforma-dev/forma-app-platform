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
        id: "dining-tracker",
        title: "Dining tracker",
        industry: "Personal",
        category: "Trackers",
        description: "Track meals, restaurants, spending, and a visible preset dining budget.",
    },
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
            "dining-tracker" => (
                "meal",
                "meals",
                json!([
                    {"id":"restaurant","value_type":{"type":"text"},"nullable":false},
                    {"id":"amount","value_type":{"type":"money","currency":"USD","minor_unit_exponent":2},"nullable":false},
                    {"id":"date","value_type":{"type":"date"},"nullable":false},
                    {"id":"notes","value_type":{"type":"text"},"nullable":true}
                ]),
                json!(["restaurant", "amount", "date", "notes"]),
            ),
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
        let mut content = json!({
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
        });
        if matches!(self.id, "dining-tracker" | "equipment-requests") {
            content["presentation"] = json!({"version":1,"description":self.description,"sections":[{"id":"overview","components":["guide"]}],"actions":[{"id":"primary","label":if self.id=="dining-tracker" {"Add meal"} else {"Request equipment"},"behavior":"dialog","target":"add"}],"theme":{"mode":"default","accent":if self.id=="dining-tracker" {"green"} else {"cyan"},"density":"comfortable","font":"default"}});
        }
        if self.id == "dining-tracker" {
            let views = content["views"].as_array_mut().unwrap();
            for (id, kind, title, extra) in [
                (
                    "ledger",
                    "records_list",
                    "Meals",
                    json!({"columns":["restaurant","amount","date","notes"],"limit":50}),
                ),
                (
                    "total",
                    "record_summary",
                    "Total spending",
                    json!({"measure":{"op":"sum","field":"amount"}}),
                ),
                (
                    "monthly",
                    "record_trend",
                    "Monthly spending",
                    json!({"measure":{"op":"sum","field":"amount"},"date_field":"date"}),
                ),
                (
                    "restaurants",
                    "record_ranking",
                    "By restaurant",
                    json!({"measure":{"op":"sum","field":"amount"},"group_field":"restaurant","limit":5}),
                ),
                (
                    "budget",
                    "record_budget",
                    "Preset dining budget",
                    json!({"measure":{"op":"sum","field":"amount"},"budget":{"origin":"preset","minor_units":"10000","currency":"USD","minor_unit_exponent":2}}),
                ),
            ] {
                let mut records = json!({"version":1,"input_id":input,"schema_id":schema});
                records
                    .as_object_mut()
                    .unwrap()
                    .extend(extra.as_object().unwrap().clone());
                views.push(json!({"id":id,"kind":kind,"renderer_version":1,"title":title,"records":records}));
            }
            content["views"][1]["text"]=json!("Record a meal to see actual spending. The budget is a preset USD 100 comparison. Filters apply to every spending component; date ranges include the start and exclude the end.");
            content["default_layout"] = json!([
                {"view_id":"guide","column":0,"row":0,"width":12},
                {"view_id":"total","column":0,"row":1,"width":6},
                {"view_id":"budget","column":6,"row":1,"width":6},
                {"view_id":"monthly","column":0,"row":2,"width":6},
                {"view_id":"restaurants","column":6,"row":2,"width":6},
                {"view_id":"ledger","column":0,"row":3,"width":12},
                {"view_id":"add","column":0,"row":4,"width":12}
            ]);
            content["presentation"]["sections"] = json!([
                {"id":"overview","components":["guide","total","budget"]},
                {"id":"insights","title":"Spending patterns","components":["monthly","restaurants"]},
                {"id":"history","title":"Meal history","components":["ledger"]}
            ]);
            let targets = json!(["ledger", "total", "monthly", "restaurants", "budget"]);
            content["record_filters"] = json!([
                {"id":"restaurant_filter","input_id":input,"field":"restaurant","kind":"text_contains","targets":targets},
                {"id":"date_filter","input_id":input,"field":"date","kind":"date_range","targets":targets}
            ]);
        }
        content
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
    #[test]
    fn composed_contract_roundtrip_and_closed_negative_vectors() {
        let template = find("dining-tracker").unwrap();
        let content = template.content();
        let org = "00000000-0000-4000-8000-000000000001";
        let app = "00000000-0000-4000-8000-000000000011";
        let admitted = plan::admit(&content, org, app).unwrap();
        assert_eq!(
            plan::admit(
                &crate::admission::decode(&admitted.canonical).unwrap(),
                org,
                app
            )
            .unwrap(),
            admitted
        );
        for (pointer, value) in [
            ("/presentation/actions/0/target", json!("foreign")),
            ("/presentation/actions/0/behavior", json!("navigate")),
            ("/presentation/actions/0/label", json!("   ")),
            ("/presentation/actions", json!([])),
            ("/presentation/theme/font", json!("remote_font")),
            ("/views/3/records/measure/field", json!("restaurant")),
            ("/views/2/records/input_id", json!("foreign")),
            ("/record_filters/0/targets", json!(["add"])),
            ("/record_filters/0/kind", json!("date_range")),
            ("/views/6/records/budget/minor_units", json!("-1")),
        ] {
            let mut value_of = content.clone();
            *value_of.pointer_mut(pointer).unwrap() = value;
            assert!(
                plan::admit(&value_of, org, app).is_err(),
                "accepted {pointer}"
            );
        }
        let mut executable = content.clone();
        executable["presentation"]["actions"][0]["href"] = json!("https://example.invalid");
        assert!(plan::admit(&executable, org, app).is_err());
        let mut duplicate = content.clone();
        duplicate["presentation"]["sections"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":"copy","components":["total"]}));
        assert!(plan::admit(&duplicate, org, app).is_err());
        let mut reference = content.clone();
        let field = json!({"id":"related","value_type":{"type":"reference","target_input":"meals"},"nullable":true});
        reference["inputs"][0]["fields"]
            .as_array_mut()
            .unwrap()
            .push(field.clone());
        reference["record_schemas"][0]["fields"]
            .as_array_mut()
            .unwrap()
            .push(field);
        reference["views"][2]["records"]["order"] = json!({"field":"related","direction":"asc"});
        assert!(plan::admit(&reference, org, app).is_err());
        let mut zero = content;
        zero["views"][6]["records"]["budget"]["minor_units"] = json!("0");
        assert!(plan::admit(&zero, org, app).is_ok());
    }
}
