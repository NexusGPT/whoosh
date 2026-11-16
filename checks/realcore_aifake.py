#!/usr/bin/env python3
"""A local AI provider for checks/realcore-check.py --slice3b: the chat/completions shape of the providers table's LM Studio, on
127.0.0.1 only, nothing leaves the Mac.

The rig core (WHOOSH_CORE_CHECK_AI=<port>) posts one chat message per message that step 1 of an AI-on label picked (A140; the check's seed: Renewals):

    POST /chat/completions   {"model", "messages": [{"role": "system", ...}, {"role": "user", "content": <the decision crate's request>}],
                              "max_tokens"}   with  Authorization: Bearer <the rig's dummy key>

The user content is the decision crate's own JSON text: {"state": "from: ..\\nsubject: ..\\n\\n<body>", "model", "questions": {<id>:
{"type": "choice", "instructions": "<question>\\n<label path>: <meaning>\\n...", "criteria": {<label path>: <label path>, ...}}}}.

The fake answers with the label path (a key of the question's criteria) whose meaning line shares a word of five letters or more with
the message's own text (the state, never the instructions), else the first candidate, at confidence 0.97, as the decision crate's
`answers` object inside the usual choices[0].message.content. It records every request in `server.seen` (the key is never kept: only
whether a bearer came) and answers GET /models with one model id.

It is also the key providers' stand-in for the first-run check (rig-core R2: the rig core with WHOOSH_CORE_CHECK_PROVIDERS=<port>
sends every request for OpenAI's or Anthropic's host here, path kept), with each provider's own shapes:

    GET  /v1/models      Anthropic (x-api-key) {"data": [{"type": "model", "id", ...}], "has_more": false, ...}
                         OpenAI (Authorization: Bearer) {"object": "list", "data": [{"id", "object": "model", ...}]}
    POST /v1/messages    Anthropic {"content": [{"type": "text", "text"}], ...}
    POST /v1/responses   OpenAI {"output": [{"type": "message", "content": [{"type": "output_text", "text"}]}], ...}

    POST /v1/systemone   Jev (Authorization: Bearer; fix-jevrig: the rig core sends Jev's host here too, path kept) takes the decision crate's
                         {"state", "model", "questions": {<id>: {"type", ...}}} and answers {"model": "jev-latest", "answers": {<id>: ...}}
                         (choice: {"choice", "confidence"}; noul: {"noul"}; score: {"score"}); a refused key answers 401
                         {"error": {"message": JEV_REFUSAL}}. Each is recorded in `server.jev_seen` as {path, accepted, questions}, never the key.

`sk-rig-good` is accepted and lists GOOD_MODELS; any other key (`sk-rig-bad` among them) answers 401 in the provider's own words
(Anthropic: "invalid x-api-key"). Each provider request is recorded in `server.provider_seen` as {path, provider, model, accepted}: the
key itself is never kept. A completion answers "A reply from <model>." so a check can see which model the core used.

    python3 checks/realcore_aifake.py --selftest   drive the server with Python's own urllib
    python3 checks/realcore_aifake.py --serve      serve on a free port: prints `port <n>`, stops when stdin closes
"""
import http.server
import json
import re
import sys
import threading

MODEL = "whoosh-check-model"
MIN_WORD = 5            # a fake's own matching rule, not a ledger number
CONFIDENCE = 0.97       # above any trust level the ledger sets for the Labels entry
GOOD_KEY = "sk-rig-good"
BAD_KEY = "sk-rig-bad"
GOOD_MODELS = ["model-a", "model-b"]
ANTHROPIC_REFUSAL = "invalid x-api-key"
JEV_REFUSAL = "Invalid API key: Jev did not accept that key."
OPENAI_REFUSAL = "Incorrect API key provided. You can find your API key at your provider's settings page."


def words(text):
    return {w for w in re.split(r"[^0-9a-zA-Z]+", text.lower()) if len(w) >= MIN_WORD}


def read_ask(body):
    """(system, user text, the decision request as a dict) of one chat body; the dict is {} when the user text is not JSON."""
    messages = body.get("messages") or []
    system = next((m.get("content") for m in messages if m.get("role") == "system"), "")
    user = next((m.get("content") for m in messages if m.get("role") == "user"), "")
    try:
        ask = json.loads(user)
    except (TypeError, ValueError):
        ask = {}
    return system, user, ask if isinstance(ask, dict) else {}


def meaning_of(instructions, option):
    """The meaning on the line "<option>: <meaning>" of the question's instructions, "" when the option has none."""
    for line in instructions.splitlines():
        if line.startswith(option + ": "):
            return line[len(option) + 2:]
    return ""


STATEMENT = 'This email belongs under the label "'   # core/crates/decision/src/decide.rs label_statement (lane fix-aisort round 3)
NO = 0.05                                             # a no for a label the fake does not pick (decision::test_answers::FAKE_NO)


def statement_label(text):
    """(label path, meaning) of a per-label yes/no statement, None for any other text."""
    if not (text or "").startswith(STATEMENT):
        return None
    rest = text[len(STATEMENT):]
    if '": ' not in rest:
        return None
    label, meaning = rest.split('": ', 1)
    return label, meaning


def asked_labels(ask):
    """(labels offered, "<label>: <meaning>" lines) of the Labels question in either form: one choice (its criteria and instruction lines),
    or one yes/no per label (`labels/model#<label>`, each statement names a label and its meaning)."""
    labels, meanings = [], []
    for q in (ask.get("questions") or {}).values():
        q = q if isinstance(q, dict) else {}
        named = statement_label(q.get("instructions") or "")
        if named:
            labels.append(named[0]); meanings.append("%s: %s" % named)
        elif q.get("type", "choice") == "choice":
            labels += list((q.get("criteria") or {}).keys()); meanings += (q.get("instructions") or "").splitlines()[1:]
    return labels, meanings


def pick(ask):
    """{question id: {"choice", "confidence"}}: the candidate whose meaning shares a word with the message, else the first. Round 3: the
    per-label yes/no questions get the same pick over the labels they name: {"noul": CONFIDENCE} for it, {"noul": NO} for the others."""
    state = ask.get("state") or ""
    mine = words(state)
    out = {}
    per_label = [(qid, statement_label((q or {}).get("instructions") or "")) for qid, q in (ask.get("questions") or {}).items() if isinstance(q, dict) and q.get("type") == "noul"]
    # the same order as the one choice's options (the label paths, sorted: decide.rs `question_for`): the question ids carry the label ids,
    # which the engine's JSON object sorts by id, not by name (round 8: main100's --slice-aisort accepted the second mail under Renewals because
    # Renewals' generated id sorted first)
    per_label = sorted(((qid, named) for qid, named in per_label if named), key=lambda x: x[1][0])
    chosen = next((named[0] for _, named in per_label if words(named[1]) & mine), per_label[0][1][0] if per_label else None)
    for qid, named in per_label:
        out[qid] = {"noul": CONFIDENCE if named[0] == chosen else NO}
    for qid, q in (ask.get("questions") or {}).items():
        if qid in out:
            continue
        options = list((q.get("criteria") or {}).keys())
        hit = next((o for o in options if words(meaning_of(q.get("instructions") or "", o)) & mine), None)
        out[qid] = {"choice": hit or (options[0] if options else ""), "confidence": CONFIDENCE}
    return out


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a):   # the check's output is its own
        pass

    def send_json(self, code, obj):
        raw = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(raw)

    def provider_ask(self):
        """(provider name, key accepted) of a provider request: Anthropic sends x-api-key, OpenAI a bearer."""
        if self.headers.get("x-api-key") is not None:
            return "anthropic", self.headers.get("x-api-key") == GOOD_KEY
        auth = self.headers.get("Authorization") or ""
        return "openai", auth == "Bearer " + GOOD_KEY

    def refuse(self, provider):
        if provider == "anthropic":
            return self.send_json(401, {"type": "error", "error": {"type": "authentication_error", "message": ANTHROPIC_REFUSAL}})
        self.send_json(401, {"error": {"message": OPENAI_REFUSAL, "type": "invalid_request_error", "code": "invalid_api_key"}})

    def provider_get(self):
        provider, ok = self.provider_ask()
        self.server.provider_seen.append({"path": self.path, "provider": provider, "model": None, "accepted": ok})
        if not ok:
            return self.refuse(provider)
        if provider == "anthropic":
            data = [{"type": "model", "id": m, "display_name": m, "created_at": "2026-01-01T00:00:00Z"} for m in GOOD_MODELS]
            return self.send_json(200, {"data": data, "has_more": False, "first_id": GOOD_MODELS[0], "last_id": GOOD_MODELS[-1]})
        self.send_json(200, {"object": "list", "data": [{"id": m, "object": "model", "created": 1767225600, "owned_by": "rig"} for m in GOOD_MODELS]})

    def provider_post(self, raw):
        provider, ok = self.provider_ask()
        try:
            body = json.loads(raw or b"{}")
        except ValueError:
            body = {}
        model = body.get("model")
        self.server.provider_seen.append({"path": self.path, "provider": provider, "model": model, "accepted": ok})
        if not ok:
            return self.refuse(provider)
        text = "A reply from %s." % model
        if self.path == "/v1/messages":
            return self.send_json(200, {"id": "msg_rig", "type": "message", "role": "assistant", "model": model, "content": [{"type": "text", "text": text}], "stop_reason": "end_turn"})
        self.send_json(200, {"id": "resp_rig", "object": "response", "model": model, "output": [{"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": text}]}]})

    def jev_post(self, raw):
        """Jev's wire (core/crates/decision/src/adapters/wire.rs): the request {state, model, questions}, the answer {model, answers}."""
        try:
            body = json.loads(raw or b"{}")
        except ValueError:
            body = {}
        questions = body.get("questions") if isinstance(body.get("questions"), dict) else {}
        ok = (self.headers.get("Authorization") or "") == "Bearer " + GOOD_KEY
        self.server.jev_seen.append({"path": self.path, "accepted": ok, "questions": list(questions.keys())})
        if not ok:
            return self.send_json(401, {"error": {"message": JEV_REFUSAL}})
        answers = {}
        for qid, q in questions.items():
            kind = q.get("type") if isinstance(q, dict) else None
            if kind == "noul":
                answers[qid] = {"noul": 0.9}
            elif kind == "score":
                answers[qid] = {"score": 0.5}
            else:
                options = list(((q or {}).get("criteria") or {}).keys()) if isinstance(q, dict) else []
                answers[qid] = {"choice": options[0] if options else "", "confidence": CONFIDENCE}
        self.send_json(200, {"model": "jev-latest", "answers": answers})

    def do_GET(self):
        if self.path == "/v1/models":
            return self.provider_get()
        if self.path == "/models":
            return self.send_json(200, {"data": [{"id": MODEL}]})
        self.send_json(404, {"error": {"message": "no such path " + self.path}})

    def do_POST(self):
        n = int(self.headers.get("Content-Length") or 0)
        raw = self.rfile.read(n) if n else b""
        if self.path in ("/v1/messages", "/v1/responses"):
            return self.provider_post(raw)
        if self.path == "/v1/systemone":
            return self.jev_post(raw)
        if self.path != "/chat/completions":
            return self.send_json(404, {"error": {"message": "no such path " + self.path}})
        try:
            body = json.loads(raw or b"{}")
        except ValueError:
            return self.send_json(400, {"error": {"message": "the body is not JSON"}})
        system, user, ask = read_ask(body)
        questions = ask.get("questions") or {}
        auth = self.headers.get("Authorization") or ""
        first = next(iter(questions.values()), {})
        self.server.seen.append({
            "path": self.path,
            "bearer": auth.startswith("Bearer ") and len(auth) > len("Bearer "),
            "model": body.get("model"),
            "system": system,
            "state": ask.get("state") or "",
            "instructions": first.get("instructions") or "",
            "options": list((first.get("criteria") or {}).keys()),
            "questions": list(questions.keys()),
            "labels": asked_labels(ask)[0],          # round 3: the labels offered, in either form of the Labels question
            "meanings": asked_labels(ask)[1],
        })
        answer = {"answers": pick(ask)}
        self.send_json(200, {"choices": [{"message": {"role": "assistant", "content": json.dumps(answer)}}]})


class Server(http.server.ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = True


def start(port=0):
    """The provider on 127.0.0.1 in a background thread; raises OSError when the port cannot be bound. `.seen` lists every chat request."""
    server = Server(("127.0.0.1", port), Handler)
    server.seen = []
    server.provider_seen = []
    server.jev_seen = []
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def selftest():
    import urllib.error
    import urllib.request
    server = start(0)
    base = "http://127.0.0.1:%d" % server.server_address[1]
    try:
        def chat(state, options, meanings, key="dummy"):
            instructions = "Which label?" + "".join("\n%s: %s" % (o, m) for o, m in meanings.items())
            ask = {"state": state, "model": MODEL, "questions": {"labels/model": {"type": "choice", "instructions": instructions, "criteria": {o: o for o in options}}}}
            req = urllib.request.Request(base + "/chat/completions", method="POST", headers={"Content-Type": "application/json", "Authorization": "Bearer " + key},
                                         data=json.dumps({"model": MODEL, "messages": [{"role": "system", "content": "s"}, {"role": "user", "content": json.dumps(ask)}], "max_tokens": 9}).encode())
            with urllib.request.urlopen(req, timeout=5) as r:
                content = json.loads(r.read())["choices"][0]["message"]["content"]
            return json.loads(content)["answers"]["labels/model"]
        # A140: the engine offers the AI only the AI-on labels whose step 1 picked the mail: the check's seed has Renewals (Contracts and Answered have their AI card off)
        options = ["Answered", "Renewals", "Updates"]
        meanings = {"Answered": "mail I have answered", "Renewals": "mail about contract renewals to read and countersign"}
        by_word = chat("from: a\nsubject: Orbit renewal terms\n\nPlease countersign the new terms today.", options, meanings)
        none = chat("from: a\nsubject: Lunch\n\nFriday at noon?", options, meanings)
        instructions_only = chat("from: a\nsubject: Friday plans\n\nSee you then.", ["Zeta", "Contracts"], {"Contracts": "hello noon"})
        with urllib.request.urlopen(base + "/models", timeout=5) as r:
            models = json.loads(r.read())
        try:
            urllib.request.urlopen(base + "/elsewhere", timeout=5)
            not_found = False
        except urllib.error.HTTPError as e:
            not_found = e.code == 404
        one = server.seen[0] if server.seen else {}

        def provider(path, key, anthropic, body=None):
            headers = {"Content-Type": "application/json"}
            headers.update({"x-api-key": key, "anthropic-version": "2023-06-01"} if anthropic else {"Authorization": "Bearer " + key})
            req = urllib.request.Request(base + path, method="POST" if body is not None else "GET", headers=headers, data=json.dumps(body).encode() if body is not None else None)
            try:
                with urllib.request.urlopen(req, timeout=5) as r:
                    return r.status, json.loads(r.read())
            except urllib.error.HTTPError as e:
                return e.code, json.loads(e.read())
        a_models = provider("/v1/models", GOOD_KEY, True)
        o_models = provider("/v1/models", GOOD_KEY, False)
        a_bad = provider("/v1/models", BAD_KEY, True)
        o_bad = provider("/v1/models", BAD_KEY, False)
        a_msg = provider("/v1/messages", GOOD_KEY, True, {"model": "model-b", "max_tokens": 9, "messages": [{"role": "user", "content": "hi"}]})
        o_resp = provider("/v1/responses", GOOD_KEY, False, {"model": "model-a", "input": "hi", "max_output_tokens": 9})
        a_msg_bad = provider("/v1/messages", BAD_KEY, True, {"model": "model-b", "messages": []})
        jev_ask = {"state": "from: a\n\nhi", "model": "jev-latest", "questions": {"which_tab": {"type": "choice", "instructions": "Which tab?", "criteria": {"Primary": "Primary", "Updates": "Updates"}},
                                                                                "urgent": {"type": "noul", "instructions": "Urgent?", "criteria": {"true": "yes", "false": "no"}}}}
        jev_good = provider("/v1/systemone", GOOD_KEY, False, jev_ask)
        jev_bad = provider("/v1/systemone", BAD_KEY, False, jev_ask)
        ok = [
            ("keyword in the message picks the label whose meaning shares it", by_word == {"choice": "Renewals", "confidence": CONFIDENCE}),
            ("no shared word picks the first candidate", none["choice"] == "Answered"),
            ("a word only in the meaning, not in the message, is no match (the first candidate)", instructions_only["choice"] == "Zeta"),
            ("every chat request is recorded with its state, instructions and options", len(server.seen) == 3 and one.get("options") == options and "Renewals: mail about contract renewals" in one.get("instructions", "") and "renewal terms" in one.get("state", "")),
            ("the bearer is noted, the key is not kept", one.get("bearer") is True and "dummy" not in json.dumps(server.seen)),
            ("GET /models lists one model and another path is 404", models["data"][0]["id"] == MODEL and not_found),
            ("the good key lists model-a, model-b in Anthropic's shape", a_models[0] == 200 and [m["id"] for m in a_models[1]["data"]] == GOOD_MODELS and a_models[1]["has_more"] is False),
            ("the good key lists model-a, model-b in OpenAI's shape", o_models[0] == 200 and o_models[1]["object"] == "list" and [m["id"] for m in o_models[1]["data"]] == GOOD_MODELS),
            ("sk-rig-bad answers 401 in each provider's words", a_bad == (401, {"type": "error", "error": {"type": "authentication_error", "message": ANTHROPIC_REFUSAL}}) and o_bad[0] == 401 and o_bad[1]["error"]["message"] == OPENAI_REFUSAL),
            ("a completion names the model it was asked for, in each shape", a_msg[1]["content"][0]["text"] == "A reply from model-b." and o_resp[1]["output"][0]["content"][0]["text"] == "A reply from model-a."),
            ("a refused key gets no completion", a_msg_bad[0] == 401),
            ("provider requests are recorded without the key", len(server.provider_seen) == 7 and server.provider_seen[4] == {"path": "/v1/messages", "provider": "anthropic", "model": "model-b", "accepted": True}
             and GOOD_KEY not in json.dumps(server.provider_seen) and BAD_KEY not in json.dumps(server.provider_seen)),
            ("provider requests are not chat requests", len(server.seen) == 3),
            ("Jev's good key answers {model, answers} with one answer per question id, in each question's shape",
             jev_good[0] == 200 and jev_good[1]["model"] == "jev-latest" and jev_good[1]["answers"] == {"which_tab": {"choice": "Primary", "confidence": CONFIDENCE}, "urgent": {"noul": 0.9}}),
            ("Jev's bad key answers 401 with words that do not echo the key", jev_bad == (401, {"error": {"message": JEV_REFUSAL}}) and BAD_KEY not in json.dumps(jev_bad)),
            ("Jev requests are recorded at Jev's path without the key and are not provider requests",
             server.jev_seen == [{"path": "/v1/systemone", "accepted": True, "questions": ["which_tab", "urgent"]}, {"path": "/v1/systemone", "accepted": False, "questions": ["which_tab", "urgent"]}]
             and len(server.provider_seen) == 7 and GOOD_KEY not in json.dumps(server.jev_seen) and BAD_KEY not in json.dumps(server.jev_seen)),
        ]
    finally:
        server.shutdown()
    for name, good in ok:
        print("%-5s %s" % ("PASS" if good else "FAIL", name))
    print("selftest aifake: %s" % ("all pass" if all(g for _, g in ok) else "FAILS"))
    return all(g for _, g in ok)


def serve():
    """`--serve`: the fake on a free port of 127.0.0.1; prints `port <n>` and runs until its stdin closes (the engine test's child)."""
    server = start(0)
    print("port %d" % server.server_address[1], flush=True)
    sys.stdin.read()
    server.shutdown()
    return True


if __name__ == "__main__":
    if "--serve" in sys.argv:
        sys.exit(0 if serve() else 1)
    sys.exit(0 if "--selftest" in sys.argv and selftest() else 1)
