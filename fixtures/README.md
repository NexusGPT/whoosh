# Fixtures

- `docs/design/fixtures/mac-main-sample.json` is the design fixture: the shape of the owner's inbox with fictional people. `FixtureCore` (`--fixture-core`) serves it, and the journeys read their expected values from it.
- `fixtures/schema/mac-main-sample.schema.json` is its JSON Schema; `python3 tools/check-fixture.py` validates the fixture against the fields `FixtureCore` decodes (exit 1 on a missing or wrong-typed field).
- `fixtures/tests-j<N>.json` belong to the tests lanes.
