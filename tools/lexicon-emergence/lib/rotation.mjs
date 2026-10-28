// Which task families a condition's subjects solve in a generation, and which
// of them the next lexicon may be chosen from (Phase 4).
//
// Without "rotation" in design.json every generation solves "families" and
// all of them count. With it, generation g trains on rotation.train[g mod n]
// (a condition named in rotation.fixed always on rotation.train[0]) and also
// solves rotation.heldOut, whose results are graded and reported but never
// rank a Word for the next lexicon: they measure whether the lexicon carries
// over to tasks it was not chosen on. Phase 3's C-open lexicon filled with
// Words that each solved one task outright, and a run that keeps the same
// tasks every generation cannot tell that from a good vocabulary.

/** { train, heldOut, all } family names, `all` in the order the subject reads them. */
export function familyNames(design, condition, generation) {
  const rotation = design.rotation;
  if (!rotation) {
    const train = design.families ?? ['stats', 'text'];
    return { train, heldOut: [], all: [...train] };
  }
  const sets = rotation.train;
  if (!Array.isArray(sets) || sets.length === 0 || !sets.every((s) => Array.isArray(s) && s.length > 0)) {
    throw new Error('design.rotation.train must be a non-empty list of non-empty family lists');
  }
  const heldOut = rotation.heldOut ?? [];
  const fixed = (rotation.fixed ?? []).includes(condition);
  const train = sets[fixed ? 0 : Number(generation) % sets.length];
  const overlap = train.filter((f) => heldOut.includes(f));
  if (overlap.length) throw new Error(`families both trained on and held out: ${overlap.join(', ')}`);
  // By name, so held-out families are not always the last the subject reads.
  return { train, heldOut, all: [...train, ...heldOut].sort() };
}

/** Every family a run's design names, for analysis across generations. */
export function allFamilyNames(design) {
  if (!design.rotation) return design.families ?? ['stats', 'text'];
  return [...new Set([...design.rotation.train.flat(), ...(design.rotation.heldOut ?? [])])].sort();
}
