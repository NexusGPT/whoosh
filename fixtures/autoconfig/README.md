# fixtures/autoconfig

Read by core/crates/autoconfig/tests through fake ports; nothing here is fetched during a test.

| File | What it is | Recorded or hand-written |
|---|---|---|
| ispdb-gmail.com.xml, ispdb-outlook.com.xml, ispdb-icloud.com.xml, ispdb-yahoo.com.xml | provider database answers (HTTP 200) | recorded 2026-11-08 by core/crates/autoconfig/examples/ispdb_capture.rs: byte copies of _captures/core-autoconfig/ispdb-<name>.xml |
| ispdb-fastmail.com.404.html | the provider database's answer for fastmail.com: HTTP 404, no entry | recorded the same way |
| srv-domain.txt | a domain with SRV records only | hand-written |
| guess-only.txt | a domain where only the guess answers | hand-written |
| autodiscover-pox.xml, autodiscover-m365.json | the two Autodiscover answers | hand-written |
