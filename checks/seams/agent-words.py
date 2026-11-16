#!/usr/bin/env python3
"""Seam con-seams-s3/agent-words: the allowed-action words on the agent card are the words of the Ask panel's
refusal, and both are the contract's fixed set (C-J5.3 "words from a fixed set: ...")."""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from seamlib import *  # noqa: E402,F401,F403
from seamlib import NJ, nj, pick, equal, fixture, constant_row, constant_int, contract_row, contains_words, norm_words, run  # noqa: E402


def first(rows, field):
    if nj(rows):
        return rows
    return rows[0].get(field, NJ("no field %s" % field)) if rows else NJ("region empty")


def setk(path, val):
    def f(o):
        d = o
        for k in path[:-1]:
            d = d[k]
        d[path[-1]] = val
        return o
    return f

import re

def expected():
    m = re.search(r"words from a fixed set: ([^|]+?) \|", contract_row("C-J5.3"))
    return {"words": [w.strip() for w in m.group(1).split(",")]}

def observe(c, exp):
    card = c.rows("agent_card")
    c.act("key", "⌘K"); c.act("type", "ask"); c.act("key", "Enter")
    c.act("type", "delete every mail from Linea Supply"); c.act("key", "Enter")  # an action outside the set
    ref = c.rows("ask_refusal")
    return {"agent-card": first(card, "allowed"), "ask-refusal": first(ref, "allowed")}

def judge(o, exp):
    return equal("allowed-words", sorted(exp["words"]), {s: (v if nj(v) else sorted(v)) for s, v in o.items()})

def oracle(exp):
    return {"agent-card": list(exp["words"]), "ask-refusal": list(exp["words"])}

# MUTANTS
# enum-names  | the refusal lists internal enum names (send_confirmed)              | allowed-words@ask-refusal
# card-drops  | the card drops "send with your confirmation"                        | allowed-words@agent-card
MUTANTS = {
    "enum-names": ("internal names", "allowed-words", setk(["ask-refusal"], ["read_mail", "search", "draft", "label", "send_confirmed"])),
    "card-drops": ("missing word", "allowed-words", lambda o: (o["agent-card"].pop() and o)),
}
if __name__ == "__main__":
    sys.exit(run("agent-words", expected, observe, judge, oracle, MUTANTS))
