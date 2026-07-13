import { verifyFixtureSet } from "./fixture-policy.mjs";

const result = await verifyFixtureSet();
console.log(
  `bmad-fixtures: verified ${result.descriptorCount} sealed descriptors`,
);
