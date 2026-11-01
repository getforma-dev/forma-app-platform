"""APP-158/159 closed, bounded projections over an authorized record snapshot.

No I/O, SQL, network, expressions, formulas, collection selectors or write path.
The parent captures the exact work snapshot and runs this under the same queue,
empty-environment child confinement and deadline as published source queries.
"""
from __future__ import annotations
import datetime
import re
import time

MAX_ROWS = 2000
MAX_BYTES = 1048576
INTEGER = re.compile(r"(?:0|-[1-9][0-9]{0,37}|[1-9][0-9]{0,37})\Z")
KINDS = {"records_list", "record_summary", "record_trend", "record_ranking", "record_budget"}

class Refused(Exception):
    pass

def require(condition, reason="record_projection_refused"):
    if not condition:
        raise Refused(reason)

def exact(obj, required, optional=()):
    require(isinstance(obj, dict) and set(required) <= set(obj) <= set(required) | set(optional))

def integer(value):
    require(isinstance(value, str) and INTEGER.fullmatch(value) is not None, "integer_overflow")
    return int(value)

def date(value):
    require(isinstance(value, str) and re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}",value))
    try:
        return datetime.date.fromisoformat(value)
    except ValueError:
        raise Refused("date_refused") from None

def scalar(value):
    require(isinstance(value, dict) and "type" in value)
    kind=value["type"]
    if kind == "null":
        exact(value,["type"]); return None
    if kind == "money":
        exact(value,["type","minor_units","currency","minor_unit_exponent"])
        return integer(value["minor_units"])
    if kind in ("integer", "decimal"):
        return integer(value.get("value",value.get("coefficient")))
    result=value.get("value")
    require(isinstance(result,(str,bool)), "value_refused")
    if kind == "date": date(result)
    return result

def values(row):
    require(isinstance(row["values"],list))
    result={}
    for item in row["values"]:
        exact(item,["field_id","value"])
        require(item["field_id"] not in result)
        result[item["field_id"]]=item["value"]
    return result

def execute(request, start=None):
    start=time.monotonic() if start is None else start
    exact(request,["kind","payload","parameters","deadline_ms"])
    require(request["kind"]=="record_components_v1")
    require(type(request["deadline_ms"]) is int and 1<=request["deadline_ms"]<=1800)
    def live():
        require(time.monotonic()-start < request["deadline_ms"]/1000, "deadline")
    payload=request["payload"]; exact(payload,["components","filters","datasets"])
    parameters=request["parameters"]; exact(parameters,["filter_epoch","filters"])
    require(type(parameters["filter_epoch"]) is int and 0<=parameters["filter_epoch"]<=9007199254740991)
    require(isinstance(parameters["filters"],list) and len(parameters["filters"])<=8)
    require(isinstance(payload["components"],list) and len(payload["components"])<=32)
    require(isinstance(payload["filters"],list) and len(payload["filters"])<=8)
    definitions={f["id"]:f for f in payload["filters"]}
    require(len(definitions)==len(payload["filters"]))
    filters={}
    for f in parameters["filters"]:
        require(isinstance(f,dict) and f.get("id") in definitions and f["id"] not in filters,"filter_refused")
        declaration=definitions[f["id"]]; kind=declaration["kind"]
        if kind=="date_range":
            exact(f,["id","start","end"]); require(date(f["start"])<date(f["end"]),"date_range_refused")
        else:
            exact(f,["id","value"]);require(isinstance(f["value"],str) and 1<=len(f["value"])<=200,"filter_refused")
        filters[f["id"]]=f
    datasets={d["input_id"]:d for d in payload["datasets"]}
    require(len(datasets)==len(payload["datasets"]) and sum(len(d["rows"]) for d in datasets.values())<=MAX_ROWS,"record_bound")
    outcomes=[]
    for component in payload["components"]:
        live();require(component["kind"] in KINDS)
        config=component["records"]; dataset=datasets.get(config["input_id"])
        require(dataset is not None and dataset["schema_id"]==config["schema_id"])
        fields={f["id"]:f for f in dataset["fields"]}
        applied=[(definitions[k],v) for k,v in filters.items() if component["id"] in definitions[k]["targets"]]
        candidates=[]
        for row in dataset["rows"]:
            live();v=values(row);matches=True
            for definition,param in applied:
                require(definition["input_id"]==config["input_id"] and definition["field"] in fields)
                raw=scalar(v.get(definition["field"],{"type":"null"})); kind=definition["kind"]
                if kind=="text_contains":
                    require(fields[definition["field"]]["value_type"]["type"]=="text")
                    matches=matches and isinstance(raw,str) and param["value"].casefold() in raw.casefold()
                elif kind=="enum":
                    require(fields[definition["field"]]["value_type"]["type"]=="enum")
                    require(param["value"] in fields[definition["field"]]["value_type"]["values"],"enum_refused")
                    matches=matches and raw==param["value"]
                elif kind=="date_range":
                    require(fields[definition["field"]]["value_type"]["type"]=="date")
                    matches=matches and isinstance(raw,str) and param["start"]<=raw<param["end"]
                else: raise Refused("filter_refused")
            if matches: candidates.append((row,v))
        kind=component["kind"];count=len(candidates)
        result={"id":component["id"],"kind":kind,"population":count,"truncated_page":False,"partial":False,
                "total_scope":"full_authorized_filtered_collection","state":"ready" if count else "empty",
                "rows":[],"columns":[],"value":None,"groups":[],"budget":None}
        def measure(rows):
            m=config["measure"]
            if m["op"]=="count": return str(len(rows))
            require(m["op"]=="sum" and m["field"] in fields)
            vt=fields[m["field"]]["value_type"];require(vt["type"] in ("money","integer"))
            total=0
            for _,v in rows:
                value=v.get(m["field"],{"type":"null"})
                if value["type"]=="null": continue
                if vt["type"]=="money":
                    require(value["type"]=="money" and value["currency"]==vt["currency"] and value["minor_unit_exponent"]==vt["minor_unit_exponent"],"money_type_refused")
                    total+=integer(value["minor_units"])
                else:
                    require(value["type"]=="integer" and value["unit"]==vt["unit"],"integer_type_refused")
                    total+=integer(value["value"])
            integer(str(total));return str(total)
        if kind=="records_list":
            cols=config["columns"];require(1<=len(cols)<=16 and len(set(cols))==len(cols) and all(c in fields for c in cols))
            limit=config.get("limit",50);require(type(limit)is int and 1<=limit<=200)
            if "order" in config:
                order=config["order"];require(order["field"] in fields and order["direction"] in("asc","desc"))
                # Stable record-id tie breaker; nulls consistently last, independently of direction.
                candidates.sort(key=lambda item:item[0]["id"])
                nonnull=[item for item in candidates if scalar(item[1].get(order["field"],{"type":"null"})) is not None]
                nulls=[item for item in candidates if scalar(item[1].get(order["field"],{"type":"null"})) is None]
                nonnull.sort(key=lambda item:scalar(item[1][order["field"]]),reverse=order["direction"]=="desc")
                candidates=nonnull+nulls
            result["columns"]=[{"id":c,"value_type":fields[c]["value_type"]} for c in cols]
            result["rows"]=[{"id":row["id"],"version":row["version"],"sequence":row["sequence"],"cells":[v.get(c,{"type":"null"}) for c in cols]} for row,v in candidates[:limit]]
            result["truncated_page"]=count>limit
        else:
            result["value"]=measure(candidates)
            result["value_type"]={"type":"integer"} if config["measure"]["op"]=="count" else fields[config["measure"]["field"]]["value_type"]
            if kind in("record_ranking","record_trend"):
                field=config["group_field" if kind=="record_ranking" else "date_field"]
                require(field in fields)
                expected=("text","enum") if kind=="record_ranking" else ("date",)
                require(fields[field]["value_type"]["type"] in expected)
                groups={}
                for row,v in candidates:
                    label=scalar(v.get(field,{"type":"null"}))
                    if label is not None and kind=="record_trend": label=label[:7]
                    groups.setdefault(label,[]).append((row,v))
                # Null is its own grouping key, never a user-entered sentinel.
                grouped=[{"label":"Missing value" if label is None else label,"missing":label is None,"value":measure(rows),"count":len(rows)} for label,rows in groups.items()]
                if kind=="record_ranking":
                    limit=config.get("limit",5);require(type(limit)is int and 1<=limit<=20)
                    grouped.sort(key=lambda item:(-integer(item["value"]),item["missing"],item["label"]))
                    result["truncated_page"]=len(grouped)>limit;grouped=grouped[:limit]
                else: grouped.sort(key=lambda item:(item["missing"],item["label"]))
                result["groups"]=grouped
            elif kind=="record_budget":
                budget=config["budget"]; vt=result["value_type"]
                require(budget["origin"]=="preset" and budget["currency"]==vt.get("currency") and budget["minor_unit_exponent"]==vt.get("minor_unit_exponent"))
                denominator=integer(budget["minor_units"]);require(denominator>=0)
                spent=integer(result["value"])
                remaining=denominator-spent;integer(str(remaining))
                result["budget"]={"origin":"preset","minor_units":str(denominator),"remaining_minor_units":str(remaining),
                    "ratio_basis_points":str(spent*10000//denominator) if denominator>0 else None}
        outcomes.append(result)
    live()
    return {"outcome":"record_components","filter_epoch":parameters["filter_epoch"],"components":outcomes}
