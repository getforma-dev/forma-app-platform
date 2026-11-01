"""APP155–159 additive grammar, legacy identity and refusal vectors."""
import copy
import unittest
from validate import validate, digest, canonical, loads, ContractError

U = "00000000-0000-4000-8000-000000000001"

def definition():
    fields = [{"id":"name","value_type":{"type":"text"},"nullable":False},
              {"id":"amount","value_type":{"type":"money","currency":"USD","minor_unit_exponent":2},"nullable":False},
              {"id":"date","value_type":{"type":"date"},"nullable":False}]
    return {"kind":"app_definition","schema_version":1,"organization_id":U,"app_id":U,"revision_id":U,
            "content":{"title":"Generic app","description":"Synthetic definition","inputs":[{"id":"items","contract_id":"item_v1","contract_version":1,"kind":"records","fields":fields}],
            "record_schemas":[{"id":"item","version":1,"fields":copy.deepcopy(fields)}],
            "commands":[{"id":"add","record_schema_id":"item","operation":"create","writable_fields":["name","amount","date"],"transitions":[]}],
            "views":[{"id":"add","kind":"form","renderer_version":1,"title":"Add item","command_id":"add"},
                     {"id":"total","kind":"record_summary","renderer_version":1,"title":"Total","records":{"version":1,"input_id":"items","schema_id":"item","measure":{"op":"sum","field":"amount"}}}],
            "links":[],"default_layout":[{"view_id":"total","column":0,"row":0,"width":12},{"view_id":"add","column":0,"row":1,"width":12}],
            "presentation":{"version":1,"sections":[{"id":"main","components":["total"]}],"actions":[{"id":"create","label":"Add item","behavior":"dialog","target":"add"}],"theme":{"mode":"dark","accent":"violet","density":"compact","font":"mono"}},
            "record_filters":[{"id":"search","input_id":"items","field":"name","kind":"text_contains","targets":["total"]}]}}

def admit(value):
    value["digest"] = digest("app-definition",value["content"])
    validate(value, new_content=True)
    return value

class Presentation(unittest.TestCase):
    def test_roundtrip_immutable_identity_and_optional_defaults(self):
        value=admit(definition());self.assertEqual(admit(loads(canonical(value))),value)
        changed=copy.deepcopy(value);changed["content"]["presentation"]["theme"]["accent"]="cyan"
        self.assertNotEqual(admit(changed)["digest"],value["digest"])
        del changed["content"]["presentation"];admit(changed)

    def test_closed_actions_targets_and_reachability(self):
        for mutate in [lambda c:c["presentation"]["actions"][0].update(target="other"),
                       lambda c:c["presentation"]["actions"][0].update(behavior="navigate"),
                       lambda c:c["presentation"]["actions"][0].update(href="https://example.invalid"),
                       lambda c:c["presentation"].update(actions=[]),
                       lambda c:c["presentation"]["theme"].update(css="color:red"),
                       lambda c:c["presentation"]["sections"].append({"id":"copy","components":["total"]}),
                       lambda c:c["presentation"]["actions"][0].update(label="   ")]:
            value=definition();mutate(value["content"])
            with self.assertRaises(ContractError):admit(value)

    def test_records_scope_units_types_and_kind_specific_keys(self):
        for mutate in [lambda c:c["views"][1]["records"].update(input_id="foreign"),
                       lambda c:c["views"][1]["records"]["measure"].update(field="name"),
                       lambda c:c["views"][1]["records"].update(columns=["name"]),
                       lambda c:c["views"][1].update(command_id="add"),
                       lambda c:c["record_filters"][0].update(targets=["add"]),
                       lambda c:c["record_filters"][0].update(kind="date_range"),
                       lambda c:c["record_schemas"][0]["fields"][0].update(nullable=True)]:
            value=definition();mutate(value["content"])
            with self.assertRaises(ContractError):admit(value)

    def test_reference_columns_are_data_but_not_qualified_sort_keys(self):
        value=definition();content=value["content"]
        field={"id":"related","value_type":{"type":"reference","target_input":"items"},"nullable":True}
        content["inputs"][0]["fields"].append(field)
        content["record_schemas"][0]["fields"].append(copy.deepcopy(field))
        view=content["views"][1];view["kind"]="records_list"
        view["records"]={"version":1,"input_id":"items","schema_id":"item","columns":["name","related"]}
        admit(value)
        view["records"]["order"]={"field":"related","direction":"asc"}
        with self.assertRaises(ContractError):admit(value)

    def test_zero_budget_is_admitted_as_a_preset_not_command(self):
        value=definition();view=value["content"]["views"][1];view["kind"]="record_budget"
        view["records"]["budget"]={"origin":"preset","minor_units":"0","currency":"USD","minor_unit_exponent":2}
        admit(value)
        view["records"]["budget"]["minor_units"]="-1"
        with self.assertRaises(ContractError):admit(value)

if __name__ == '__main__': unittest.main()
