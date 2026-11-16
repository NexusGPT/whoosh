// GENERATED from docs/FIRST-RUN.json by checks/checklib/firstrun_data.py: do not edit by hand (checks/probes-check.py, firstrun-data-equals-golden).
// The values of a new installation, as the REAL core serves them; FixtureFirstRun.swift applies them to the fixture core.
enum FirstRunGolden {
    static let json = #"""
{
 "auto": false,
 "filter": {
  "ai_description": "Emails that wait for a reply from me: a question, a request, a decision that someone is waiting for.",
  "ai_enabled": true,
  "entry_enabled": true,
  "exit_enabled": true
 },
 "labels": [
  {
   "ai_enabled": true,
   "ai_reads": {
    "attachments": false,
    "body": true,
    "rest_of_thread": false,
    "subject": true
   },
   "chains": {
    "entry": [],
    "exit": [
     [
      {
       "fact": "unanswered",
       "kind": "fact",
       "wanted": true
      }
     ]
    ]
   },
   "description": "Emails that wait for a reply from me: a question, a request, a decision that someone is waiting for.",
   "entry_enabled": true,
   "exit_enabled": true,
   "exit_fallback": "primary",
   "id": "needs-reply",
   "pinned": true
  },
  {
   "ai_enabled": true,
   "ai_reads": {
    "attachments": false,
    "body": true,
    "rest_of_thread": false,
    "subject": true
   },
   "chains": {
    "entry": [],
    "exit": []
   },
   "description": "Automatic notices about my accounts and orders: receipts, shipping and delivery news, confirmations, alerts, statements.",
   "entry_enabled": true,
   "exit_enabled": false,
   "exit_fallback": null,
   "id": "updates",
   "pinned": true
  },
  {
   "ai_enabled": false,
   "ai_reads": {
    "attachments": false,
    "body": true,
    "rest_of_thread": false,
    "subject": true
   },
   "chains": {
    "entry": [
     [
      {
       "fact": "mailing_list",
       "kind": "fact",
       "wanted": true
      }
     ]
    ],
    "exit": []
   },
   "description": "Marketing and offers: newsletters, sales, discounts and promotional campaigns.",
   "entry_enabled": true,
   "exit_enabled": false,
   "exit_fallback": null,
   "id": "promotions",
   "pinned": true
  },
  {
   "ai_enabled": true,
   "ai_reads": {
    "attachments": false,
    "body": true,
    "rest_of_thread": false,
    "subject": true
   },
   "chains": {
    "entry": [],
    "exit": []
   },
   "description": "Messages from social networks and communities: invitations, mentions, new followers and activity of my contacts.",
   "entry_enabled": true,
   "exit_enabled": false,
   "exit_fallback": null,
   "id": "social",
   "pinned": false
  }
 ],
 "providers": [
  {
   "connected": false,
   "id": "openai",
   "models": [],
   "name": "OpenAI"
  },
  {
   "connected": false,
   "id": "anthropic",
   "models": [],
   "name": "Anthropic"
  },
  {
   "connected": false,
   "id": "gemini",
   "models": [],
   "name": "Google Gemini"
  },
  {
   "connected": false,
   "id": "openrouter",
   "models": [],
   "name": "OpenRouter"
  },
  {
   "connected": true,
   "id": "local",
   "models": [
    "on-device"
   ],
   "name": "On this Mac"
  }
 ],
 "refusal": {
  "code": "ai_no_model",
  "words": "No model is set for sorting mail: choose one in Settings, Decisions."
 },
 "sources": [
  {
   "account": 1,
   "folders": [
    {
     "enabled": true,
     "name": "INBOX",
     "progress": {
      "state": "up_to_date"
     }
    },
    {
     "enabled": false,
     "name": "Projects",
     "progress": {
      "state": "not_started"
     }
    },
    {
     "enabled": true,
     "name": "Éléments envoyés",
     "progress": {
      "state": "up_to_date"
     }
    }
   ]
  },
  {
   "account": 2,
   "folders": [
    {
     "enabled": true,
     "name": "INBOX",
     "progress": {
      "state": "up_to_date"
     }
    },
    {
     "enabled": false,
     "name": "Receipts",
     "progress": {
      "state": "not_started"
     }
    },
    {
     "enabled": true,
     "name": "Sent Items",
     "progress": {
      "state": "up_to_date"
     }
    }
   ]
  }
 ]
}
"""#
}
