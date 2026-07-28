import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * The half of the partition the type system cannot check.
 *
 * `DomainPartitionIsTotal` proves at compile time that every client method is
 * claimed by *at least* one domain. It cannot prove that none is claimed by
 * two: a union does not care about duplicates, so filing `chatHistory` under
 * both `ChatClient` and `ChangesClient` compiles cleanly.
 *
 * A duplicate is not harmless. The point of the partition is that a feature's
 * authority is legible from the interface it takes, and a method reachable from
 * two domains means two features hold a power that only one of them should --
 * exactly the drift the split exists to remove. So the file is read as text.
 */

/// From the package root: vitest rewrites `import.meta.url` to a non-file
/// scheme here, so it cannot be resolved back to a path.
const SOURCE = resolve(process.cwd(), "src/lib/hostClient/clients/index.ts");

/** Method names as they appear inside each `Pick<DesktopHostClient, ...>`. */
function domainMembers(): Map<string, string[]> {
  const source = readFileSync(SOURCE, "utf8");
  const domains = new Map<string, string[]>();
  const pattern = /export type (\w+) = Pick<DesktopHostClient,\s*([\s\S]*?)>;/gu;
  for (const [, name, body] of source.matchAll(pattern)) {
    if (name === undefined || body === undefined) continue;
    domains.set(name, [...body.matchAll(/"([^"]+)"/gu)].map((m) => m[1] as string));
  }
  return domains;
}

describe("the domain partition", () => {
  it("finds every domain interface", () => {
    // Guards the parser itself: a regex that matched nothing would make every
    // assertion below vacuously true.
    const domains = domainMembers();
    expect(domains.size).toBe(11);
    expect([...domains.keys()]).toContain("ChatClient");
  });

  it("claims each client method exactly once", () => {
    const owners = new Map<string, string[]>();
    for (const [domain, members] of domainMembers()) {
      for (const member of members) {
        owners.set(member, [...(owners.get(member) ?? []), domain]);
      }
    }
    const shared = [...owners.entries()]
      .filter(([, domains]) => domains.length > 1)
      .map(([method, domains]) => `${method}: ${domains.join(", ")}`);
    expect(shared).toEqual([]);
  });

  it("gives every domain at least one method", () => {
    // An empty domain is a name with no authority behind it, which reads as a
    // boundary while enforcing nothing.
    for (const [domain, members] of domainMembers()) {
      expect(members.length, `${domain} claims no methods`).toBeGreaterThan(0);
    }
  });

  it("lists no method twice inside one domain", () => {
    for (const [domain, members] of domainMembers()) {
      expect(new Set(members).size, `${domain} repeats a method`).toBe(members.length);
    }
  });

  /// The read-only domains are the ones worth stating outright: a feature that
  /// takes one of these cannot change anything, and that should stay true.
  it("keeps the read-only domains free of mutating commands", () => {
    const domains = domainMembers();
    const readOnly = ["GitClient", "BmadCapabilityClient"];
    const mutating = /^(set|enable|revoke|erase|approve|submit|decide|propose|request|prepare|cancel|send|pick|select)/u;
    for (const domain of readOnly) {
      for (const member of domains.get(domain) ?? []) {
        expect(mutating.test(member), `${domain}.${member} looks like a mutation`).toBe(false);
      }
    }
  });
});
