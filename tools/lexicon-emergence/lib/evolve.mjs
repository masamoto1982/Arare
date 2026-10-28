// Classifying Words into "the same Word" classes, and choosing the next
// generation's lexicon under the capacity bottleneck (work order §5.1).

import { identify } from './identity.mjs';
import { closure, norm, tokenize } from './source.mjs';

/** Every Word that was ever in play: lexicon entries and each submission's own definitions. */
export function collectWords(graded, lexicons) {
  const words = new Map();
  for (const lexicon of lexicons) {
    for (const entry of lexicon.entries) words.set(norm(entry.name), { ...entry, origin: 'lexicon' });
  }
  for (const g of graded) {
    for (const d of g.definitions) {
      words.set(norm(d.name), { ...d, definedBy: g.agent, generation: g.generation, condition: g.condition });
    }
  }
  return words;
}

/** Union Words that share a D0, D0α or D1 key. Returns name → { classId, link }. */
export async function classify(ajisai, words) {
  const ids = [];
  for (const name of words.keys()) ids.push(await identify(ajisai, name, words));

  const parent = new Map(ids.map((id) => [id.name, id.name]));
  const find = (x) => (parent.get(x) === x ? x : find(parent.get(x)));
  const link = new Map();
  for (const level of ['d0', 'd0a', 'd1']) {
    const first = new Map();
    for (const id of ids) {
      const key = id[level];
      if (key === null) continue;
      if (!first.has(key)) {
        first.set(key, id.name);
        continue;
      }
      const a = find(first.get(key));
      const b = find(id.name);
      if (a !== b) {
        parent.set(b, a);
        link.set(id.name, level);
      }
    }
  }
  const classes = new Map();
  for (const id of ids) {
    classes.set(id.name, { classId: find(id.name), link: link.get(id.name) ?? 'self', ...id });
  }
  return classes;
}

// A task id is `<family>.<task>`.
const familyOf = (task) => task.slice(0, task.indexOf('.'));

/** Uses per class: how many correct solutions reached it, by which agents, from which tasks and families. */
function usage(graded, classes) {
  const byClass = new Map();
  for (const g of graded) {
    for (const r of g.results) {
      if (!r.correct) continue;
      const reached = new Set(r.uses.map((name) => classes.get(name)?.classId).filter(Boolean));
      for (const classId of reached) {
        const u = byClass.get(classId) ?? { uses: 0, agents: new Set(), tasks: new Set(), families: new Set() };
        u.uses += 1;
        u.agents.add(g.agent);
        u.tasks.add(r.task);
        u.families.add(familyOf(r.task));
        byClass.set(classId, u);
      }
    }
  }
  return byClass;
}

/**
 * The top-K classes by use, each represented by its most-used member and
 * carried with the definitions that member depends on. The dependencies
 * count against K: capacity is a limit on what the next generation reads.
 *
 * A dependency whose class is already represented is rewritten to call the
 * representative instead of being carried a second time — but only when that
 * class was joined at D0 or D0α, where the two are the same Word. A class
 * joined at D1 agreed on the probes only, so its members are carried as they
 * are.
 */
/**
 * How classes are ranked for the next lexicon. `uses` (the pilot's and Phase
 * 2's rule): by how many correct solutions reached the class. `reinvention`:
 * first by how many of this generation's subjects defined a member of the
 * class themselves and reached it in a correct solution, then by uses. Phase
 * 2 found `uses` locks the lexicon in after one generation — inherited Words
 * gather uses because they are inherited — while the Words most often
 * reinvented never entered it (M-6); `reinvention` ranks those first. Phase 3
 * found `reinvention` punishes inheritance: a Word its heirs reuse is not
 * written again, falls out, is rewritten, and comes back. `breadth`: first by
 * how many distinct tasks correct solutions reached the class from, whoever
 * defined it, then by uses — a Word that solves one task outright reaches one
 * task however many subjects call it. Phase 4 found `breadth` ranks a Word
 * that reaches every task of one family (bracket encoding) with the Words
 * that cross families. `families`: first by how many distinct task families
 * reached the class, then by tasks, then by uses.
 *
 * Every rule gives a class one place at most. Phase 4's `breadth` lexicon
 * carried two means: a chosen Word depended on its own mean, which had joined
 * the class of the mean already chosen at D1 only, so it was neither rewritten
 * nor refused. A member whose dependencies would bring a second member of a
 * chosen class is now passed over for the class's next member; a class none
 * of whose members fits that way waits until every class has been tried, and
 * then takes what capacity is left, duplicates and all — so a duplicate never
 * displaces a class, and a lexicon without capacity loses no class.
 *
 * Results marked `heldOut` (rotation.mjs) never count: the next lexicon is
 * chosen from the training tasks only.
 */
export const SELECTION_RULES = ['uses', 'reinvention', 'breadth', 'families'];

export function nextLexicon(generation, capacity, graded, words, classes, rule = 'uses') {
  if (!SELECTION_RULES.includes(rule)) throw new Error(`unknown selection rule ${rule}; one of ${SELECTION_RULES.join(', ')}`);
  graded = graded.map((g) => ({ ...g, results: g.results.filter((r) => !r.heldOut) }));
  const perClass = usage(graded, classes);
  const memberUses = new Map();
  for (const g of graded) {
    for (const r of g.results) if (r.correct) for (const n of r.uses) memberUses.set(n, (memberUses.get(n) ?? 0) + 1);
  }
  const membersOf = new Map();
  for (const c of classes.values()) membersOf.set(c.classId, [...(membersOf.get(c.classId) ?? []), c]);
  const strong = (classId) => membersOf.get(classId).every((c) => c.link !== 'd1');
  const byPreference = (a, b) =>
    (memberUses.get(b) ?? 0) - (memberUses.get(a) ?? 0) ||
    tokenize(words.get(a).body).length - tokenize(words.get(b).body).length;

  const representative = new Map(); // classId → chosen name, for strong classes
  const placed = new Set(); // every class with a member in the lexicon
  const chosen = [];
  const chosenNames = new Set();
  const rewrite = (body) =>
    tokenize(body)
      .map((t) => {
        const c = classes.get(norm(t));
        return c && strong(c.classId) && representative.has(c.classId) ? representative.get(c.classId) : t;
      })
      .join(' ');
  const include = (name) => {
    const c = classes.get(name);
    if (chosenNames.has(name) || (strong(c.classId) && representative.has(c.classId))) return [];
    const plan = [];
    for (const dep of closure([name], words).slice(0, -1)) plan.push(...include(norm(dep.name)));
    return [...plan, name];
  };

  // Subjects who defined a member of a class themselves and reached it in a correct solution.
  const reinventors = new Map();
  for (const g of graded) {
    const own = new Set(g.definitions.map((d) => classes.get(norm(d.name))?.classId).filter(Boolean));
    for (const r of g.results) {
      if (!r.correct) continue;
      for (const name of r.uses) {
        const classId = classes.get(name)?.classId;
        if (classId && own.has(classId)) reinventors.set(classId, (reinventors.get(classId) ?? new Set()).add(g.agent));
      }
    }
  }
  const reinvented = (classId) => reinventors.get(classId)?.size ?? 0;
  const ranked = [...perClass.entries()].sort(
    ([ca, a], [cb, b]) =>
      (rule === 'reinvention' ? reinvented(cb) - reinvented(ca) : 0) ||
      (rule === 'families' ? b.families.size - a.families.size : 0) ||
      (rule === 'breadth' || rule === 'families' ? b.tasks.size - a.tasks.size : 0) ||
      b.uses - a.uses ||
      b.agents.size - a.agents.size,
  );
  // A plan that would give some class a second place: two of its own names in
  // one class, or a name in a class already placed.
  const duplicates = (plan) => {
    const seen = new Set();
    return plan.some((name) => {
      const classId = classes.get(name).classId;
      if (placed.has(classId) || seen.has(classId)) return true;
      seen.add(classId);
      return false;
    });
  };
  const waiting = [];
  const place = (plan) => {
    for (const name of plan) {
      const c = classes.get(name);
      const d = words.get(name);
      chosen.push({
        name: d.name,
        body: rewrite(d.body),
        note: d.note ?? '',
        classId: c.classId,
        uses: perClass.get(c.classId)?.uses ?? 0,
        tasks: perClass.get(c.classId)?.tasks.size ?? 0,
        families: perClass.get(c.classId)?.families.size ?? 0,
        agents: [...(perClass.get(c.classId)?.agents ?? [])],
      });
      chosenNames.add(name);
      placed.add(c.classId);
      if (strong(c.classId)) representative.set(c.classId, d.name);
    }
  };
  for (const [classId] of ranked) {
    if (placed.has(classId)) continue;
    const members = membersOf.get(classId).map((c) => c.name).sort(byPreference);
    const plan = members.map((m) => [...new Set(include(m))]).find((p) => !duplicates(p));
    if (!plan) waiting.push(members[0]);
    else if (chosen.length + plan.length <= capacity) place(plan);
  }
  for (const name of waiting) {
    if (placed.has(classes.get(name).classId)) continue;
    const plan = [...new Set(include(name))];
    if (chosen.length + plan.length <= capacity) place(plan);
  }
  return { generation, capacity, rule, entries: chosen };
}
