"""The part of docs/FIRST-RUN.json the fixture core serves in `fixture install-state new`, as the JSON Swift embeds (probes-r1, round 6).

ONE file both cores are compared with: docs/FIRST-RUN.json (read from the real core). `subset(golden)` cuts the field classes the fixture
serves out of it; `render(golden)` writes Sources/CoreBridge/FixtureFirstRunData.swift from that subset, byte for byte. The check
(checks/probes-check.py, case firstrun-data-equals-golden) regenerates the file and compares, so the Swift data cannot drift from the golden.
Classes: labels, providers, filter, auto, sources, refusal. Account ids are the golden's masks: account 1 and 2 map to the fixture's first two accounts.
"""
import json


def subset(g):
    d = g["defaults"]
    labels = []
    for l in d["reads"]["labels"]:
        i = l["id"]
        chains = {}
        for place in ("entry", "exit"):
            rows = d["state"].get("label/%s/chains/%s" % (i, place)) or []
            chains[place] = [c["conditions"] for c in rows]
        labels.append({"id": i, "description": l["description"], "entry_enabled": l["entry_enabled"], "ai_enabled": l["ai_enabled"],
                       "exit_enabled": l["exit_enabled"], "exit_fallback": l["exit_fallback"], "pinned": l["pinned"], "ai_reads": l["ai_reads"], "chains": chains})
    providers = [{"id": p["id"], "name": p["name"], "connected": p["connected"], "models": p["models"]} for p in d["reads"]["ai_providers"]]
    f = d["state"]["reply/filter"]
    filt = {"ai_description": f["ai"]["description"], "ai_enabled": f["ai"]["enabled"], "entry_enabled": f["entry"]["enabled"], "exit_enabled": f["exit"]["enabled"]}
    sources = []
    for acc in d["state"]["reply/sources"]["accounts"]:
        n = int(acc["account_id"].replace("$ACCOUNT_", ""))
        sources.append({"account": n, "folders": [{"name": x["source"]["folder"], "enabled": x["enabled"], "progress": x["progress"]} for x in acc["folders"]]})
    refusal = d["state"]["label/needs-reply/flow/ai_refusal"]
    return {"labels": labels, "providers": providers, "filter": filt, "auto": d["state"]["reply/use/auto"]["enabled"], "sources": sources, "refusal": refusal}


def render(g):
    body = json.dumps(subset(g), indent=1, sort_keys=True, ensure_ascii=False)
    return ('// GENERATED from docs/FIRST-RUN.json by checks/checklib/firstrun_data.py: do not edit by hand (checks/probes-check.py, firstrun-data-equals-golden).\n'
            '// The values of a new installation, as the REAL core serves them; FixtureFirstRun.swift applies them to the fixture core.\n'
            'enum FirstRunGolden {\n    static let json = #"""\n' + body + '\n"""#\n}\n')
