import type { NodeConfig } from "../types";

export function effectiveAgentLabel(agent: Pick<NodeConfig, "label">, idx: number): string {
  const trimmed = agent.label.trim();
  return trimmed || `角色 ${idx + 1}`;
}

export function nextUniqueAgentLabel(agents: Pick<NodeConfig, "label">[]): string {
  const used = new Set(agents.map((a, idx) => effectiveAgentLabel(a, idx)));
  let n = agents.length + 1;
  while (used.has(`角色 ${n}`)) {
    n += 1;
  }
  // Also fill gaps from 1 upwards if length+1 was already taken or start from 1?
  // Checking 1..N first gives intuitive numbering when gaps exist, fallback to next n:
  for (let i = 1; i <= agents.length + 1; i += 1) {
    if (!used.has(`角色 ${i}`)) {
      return `角色 ${i}`;
    }
  }
  return `角色 ${n}`;
}

export function duplicateLabelSet(agents: Pick<NodeConfig, "label">[]): Set<string> {
  const counts = new Map<string, number>();
  agents.forEach((a, idx) => {
    const lbl = effectiveAgentLabel(a, idx);
    counts.set(lbl, (counts.get(lbl) ?? 0) + 1);
  });
  const dups = new Set<string>();
  counts.forEach((cnt, lbl) => {
    if (cnt > 1) dups.add(lbl);
  });
  return dups;
}

export function findDuplicateAgentLabel(agents: Pick<NodeConfig, "label">[]): string | null {
  const seen = new Set<string>();
  for (let i = 0; i < agents.length; i += 1) {
    const lbl = effectiveAgentLabel(agents[i], i);
    if (seen.has(lbl)) return lbl;
    seen.add(lbl);
  }
  return null;
}

export function disambiguateAgentLabel(
  desired: string,
  agents: Pick<NodeConfig, "label">[],
  ignoreIdx: number
): string {
  const base = desired.trim() || `角色 ${ignoreIdx + 1}`;
  const others = new Set(
    agents
      .map((a, idx) => (idx === ignoreIdx ? null : effectiveAgentLabel(a, idx)))
      .filter((v): v is string => v !== null)
  );
  if (!others.has(base)) return base;
  let suffix = 2;
  while (others.has(`${base} (${suffix})`)) {
    suffix += 1;
  }
  return `${base} (${suffix})`;
}
