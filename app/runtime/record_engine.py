"""APP-158/159 closed, bounded projections over an authorized record snapshot.

No I/O, SQL, network, expressions, formulas, collection selectors or write path.
The parent captures the exact work snapshot and runs this under the same queue,
empty-environment child confinement and deadline as published source queries.

ADR-020 (APP-204): the countdown follows the current record of its input (the
readable row with the highest commit sequence); a ranking may group by the
row's creator, labelled from the parent's `creators` map; the `matches`
measure counts rows equal, field by field, to some row of a key input. The
parent captured every input under the viewer's own rows, so a member who may
read only their own rows is ranked alone. Principal ids enter; labels leave.
"""
from __future__ import annotations
import datetime
import json
import re
import time

MAX_ROWS = 2000
MAX_BYTES = 1048576
INTEGER = re.compile(r"(?:0|-[1-9][0-9]{0,37}|[1-9][0-9]{0,37})\Z")
TIMESTAMP = re.compile(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z\Z")
KINDS = {"records_list", "record_summary", "record_trend", "record_ranking", "record_budget", "countdown"}
COMPARISONS = ("eq", "ne", "lt", "lte", "gt", "gte")
NULL = {"type": "null"}
# ADR-020 bounds: a countdown's label, a group label, a ranking's page, an offset.
LABEL = 200
GROUP_LABEL = 2000
RANKING_LIMIT = 50
WEEK = 604800

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

def timestamp(value):
    require(isinstance(value, str) and TIMESTAMP.fullmatch(value) is not None, "timestamp_refused")
    try:
        return datetime.datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError:
        raise Refused("timestamp_refused") from None

def instant(moment):
    # Fixed width whatever the platform's strftime does with small years.
    return f"{moment.year:04d}-{moment.month:02d}-{moment.day:02d}T{moment.hour:02d}:{moment.minute:02d}:{moment.second:02d}Z"

def canonical(value):
    """ADR-020 section 3 typed equality: equal canonical forma-json-v1 bytes."""
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)

def ordered(value):
    """storage::rules `key`: money, integer and decimal by number; date,
    timestamp, text and enum by text; boolean by flag; anything else (null,
    reference) has no order and satisfies no guard."""
    require(isinstance(value, dict))
    kind = value.get("type")
    if kind == "money": return ("number", integer(value.get("minor_units")))
    if kind == "integer": return ("number", integer(value.get("value")))
    if kind == "decimal": return ("number", integer(value.get("coefficient")))
    if kind == "boolean":
        return ("flag", value["value"]) if isinstance(value.get("value"), bool) else None
    if kind in ("date", "timestamp", "text", "enum"):
        return ("text", value["value"]) if isinstance(value.get("value"), str) else None
    return None

def holds(guard, state, fields):
    """An APP-107 guard on one record, as storage::rules::Guard::holds judges it."""
    exact(guard, ["field_id", "comparison", "value"])
    require(guard["field_id"] in fields and guard["comparison"] in COMPARISONS)
    have = ordered(state.get(guard["field_id"], NULL)); want = ordered(guard["value"])
    if have is None or want is None or have[0] != want[0]:
        return False
    a, b = have[1], want[1]
    return {"eq": a == b, "ne": a != b, "lt": a < b, "lte": a <= b, "gt": a > b, "gte": a >= b}[guard["comparison"]]

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
    payload=request["payload"]; exact(payload,["components","filters","datasets","creators"])
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
    # ADR-020 C4: every row names its creator; the parent labels each creator
    # it captured (a display name or the neutral handle), never more. The map
    # is an id-sorted array: forma-json-v1 keys are local ids, not principals.
    require(isinstance(payload["creators"],list) and len(payload["creators"])<=MAX_ROWS)
    creators={};last=""
    for entry in payload["creators"]:
        exact(entry,["id","label"])
        require(isinstance(entry["id"],str) and isinstance(entry["label"],str) and 1<=len(entry["label"])<=GROUP_LABEL)
        require(entry["id"]>last);last=entry["id"]
        creators[entry["id"]]=entry["label"]
    for d in datasets.values():
        for row in d["rows"]:
            exact(row,["id","version","sequence","values","creator"])
            require(type(row["sequence"]) is int and isinstance(row["creator"],str) and row["creator"] in creators)
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
        # ADR-020 clarification 3: no filter narrows a countdown's current record.
        require(kind!="countdown" or not applied)
        result={"id":component["id"],"kind":kind,"population":count,"truncated_page":False,"partial":False,
                "total_scope":"full_authorized_filtered_collection","state":"ready" if count else "empty",
                "rows":[],"columns":[],"value":None,"groups":[],"budget":None}
        keys=None
        if kind!="countdown" and config.get("measure",{}).get("op")=="matches":
            # ADR-020 C4: rows equal, on every named field, to some key row.
            m=config["measure"];exact(m,["op","key_input","fields"])
            require(kind in ("record_summary","record_ranking"))
            key=datasets.get(m["key_input"]);require(key is not None and m["key_input"]!=config["input_id"])
            keyed={f["id"]:f for f in key["fields"]};names=m["fields"]
            require(isinstance(names,list) and 1<=len(names)<=4 and len(set(names))==len(names))
            for name in names:
                require(name in fields and name in keyed and canonical(fields[name]["value_type"])==canonical(keyed[name]["value_type"]))
                require(fields[name]["value_type"]["type"]!="multi_enum")
            keys=set()
            for row in key["rows"]:
                live();v=values(row);keys.add(tuple(canonical(v.get(name,NULL)) for name in names))
        def measure(rows):
            m=config["measure"]
            if m["op"]=="count": return str(len(rows))
            if m["op"]=="matches":
                return str(sum(1 for _,v in rows if tuple(canonical(v.get(name,NULL)) for name in m["fields"]) in keys))
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
        elif kind=="countdown":
            exact(config,["version","input_id","schema_id","timestamp_field"],["offset_seconds","closed_when"])
            field=config["timestamp_field"];require(field in fields and fields[field]["value_type"]["type"]=="timestamp")
            offset=config.get("offset_seconds",0);require(type(offset) is int and -WEEK<=offset<=WEEK)
            if "closed_when" in config: holds(config["closed_when"],{},fields)
            # ADR-020 section 3: the current record is the readable row whose
            # current revision has the highest commit sequence.
            current=max(candidates,key=lambda item:(item[0]["sequence"],item[0]["id"]),default=None)
            value=None;label=""
            if current is not None:
                row,v=current;raw=v.get(field,NULL)
                if raw["type"]!="null":
                    require(raw["type"]=="timestamp")
                    try: value=instant(timestamp(raw["value"])+datetime.timedelta(seconds=offset))
                    except OverflowError: raise Refused("timestamp_refused") from None
                    timestamp(value)
                text=next((f["id"] for f in dataset["fields"] if f["value_type"]["type"]=="text"),None)
                shown=v.get(text,NULL) if text is not None else NULL
                if shown["type"]=="text":
                    require(isinstance(shown["value"],str));label=shown["value"][:LABEL]
            closed=value is None or (current is not None and "closed_when" in config and holds(config["closed_when"],current[1],fields))
            result.update(value=value,value_type={"type":"timestamp"},label=label,closed=closed)
        else:
            result["value"]=measure(candidates)
            result["value_type"]={"type":"integer"} if config["measure"]["op"] in ("count","matches") else fields[config["measure"]["field"]]["value_type"]
            if kind=="record_ranking": require(("group_field" in config)!=("group_by" in config))
            if kind=="record_ranking" and "group_by" in config:
                # ADR-020 C4: one group per creator, labelled, never identified.
                require("group_field" not in config and config["group_by"]=="creator")
                groups={}
                for row,v in candidates: groups.setdefault(row["creator"],[]).append((row,v))
                ranked=[]
                for creator,rows in groups.items():
                    value=measure(rows)
                    # By value, then label; the creator orders equal labels stably and is dropped.
                    ranked.append(((-integer(value),creators[creator],creator),{"label":creators[creator],"missing":False,"value":value,"count":len(rows)}))
                ranked.sort(key=lambda item:item[0])
                limit=config.get("limit",5);require(type(limit)is int and 1<=limit<=RANKING_LIMIT)
                result["truncated_page"]=len(ranked)>limit;result["groups"]=[group for _,group in ranked[:limit]]
            elif kind in("record_ranking","record_trend"):
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
                    limit=config.get("limit",5);require(type(limit)is int and 1<=limit<=RANKING_LIMIT)
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
