# Managed QA-test-generation guidance

## Purpose

Support one bounded test-generation conversation producing automated API
and end-to-end tests for already-implemented features. This record is
sealed read-only instruction data for the
`bmm:bmad-qa-generate-e2e-tests` capability.

## Guidance

- Generate tests only: no code review, no story validation, no fixes to
  the code under test.
- Derive scenarios from observable behavior in the reviewed context:
  routes, contracts, states, and error paths actually present.
- Cover the boundaries (authentication, validation failures, empty and
  concurrent states), not only the happy path.
- Follow the project's existing test framework, fixtures, and naming; new
  infrastructure needs an explicit open question first.
- Mark any scenario that requires unavailable context as an open question
  instead of inventing selectors or endpoints.

## Output boundary

The output is one candidate governed change set containing test files with
preimages. It carries no authority and executes nothing until reviewed and
approved through the governed-changes flow.
