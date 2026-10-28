// The one message a subject agent receives. It carries the tasks, one
// example input per family (the grader also uses inputs the subject never
// sees, so a hard-coded answer fails), and the inherited lexicon — and
// nothing of any earlier agent's conversation (work order §5.1 step 7).

import { prelude } from './source.mjs';

/**
 * `route` is how the subject runs (work order §1.2): `subagent` for a Claude
 * Code subagent that reads SKILL.md and writes a file (the pilot), `harness`
 * for the API harness, whose system prompt already carries SKILL.md and whose
 * subject hands in through a `submit` tool, and `cli` for a Claude Code
 * subagent that runs code through `cli.mjs compute` (Phase 2): that command
 * serves the generation's lexicon through `words`, as the harness does, and
 * logs every call, so a subagent need not paste inherited definitions. The
 * tasks and the lexicon read the same every way.
 */
export function subjectPrompt({ run, condition, generation, agent }, lexicon, families, route = 'subagent', { callBudget, definitionBudget } = {}) {
  const harness = route === 'harness';
  const cli = route === 'cli';
  const genPath = `tools/lexicon-emergence/runs/${run}/${condition}/gen${generation}`;
  const path = `${genPath}/submissions/${agent}.json`;
  const work = `${genPath}/work/${agent}/`;
  const compute = `node tools/lexicon-emergence/cli.mjs compute ${run} ${condition} ${generation} ${agent}`;
  const lines = [];
  lines.push(`You are subject agent ${agent} in a language-use study. Solve the tasks below in Ajisai.`);
  lines.push('');
  lines.push('## Rules');
  if (harness) {
    lines.push('- Learn Ajisai from the reference in your instructions.');
    lines.push('- Run code only with the `compute` tool. You may also use `check` and `word_contract`.');
  } else if (cli) {
    lines.push('- Learn Ajisai from `SKILL.md` at the repository root and the Word list it points to,');
    lines.push('  `skill/words.md`. Read no other file in the repository, and no file under');
    lines.push(`  \`tools/lexicon-emergence/\` outside your own working directory \`${work}\`.`);
    lines.push('- Run code only with this command, from the repository root, on a file you write in your working');
    lines.push('  directory. It prints the engine\'s JSON result (`stackDisplay` is the final stack, bottom to top):');
    lines.push('');
    lines.push(`  \`${compute} ${work}try.ajisai\``);
    lines.push('');
    lines.push('  Do not use any other way of running Ajisai (no MCP tool, no `ajisai` binary).');
    if (callBudget === 0) {
      lines.push('- In this round you may not run code at all: the command above refuses every run. Write your');
      lines.push('  solutions from what SKILL.md and skill/words.md tell you, and check them by reasoning.');
    } else if (callBudget != null) {
      lines.push(`- You may run that command at most ${callBudget} times in all; every run counts, whatever it`);
      lines.push('  contains, and a run past the budget is refused. One run may hold many test expressions.');
    }
  } else {
    lines.push('- Learn Ajisai from `SKILL.md` at the repository root. Read no other file in the repository —');
    lines.push('  in particular nothing under `tools/lexicon-emergence/`, `docs/`, `spec/` or `rust/`.');
    lines.push('- Run code only with the `mcp__ajisai__compute` tool (load it with ToolSearch first). You may');
    lines.push('  also use `mcp__ajisai__check` and `mcp__ajisai__word_contract`.');
  }
  lines.push("- Each task's input is already on the stack when your solution runs. Your solution is the code");
  lines.push('  that follows it and must leave exactly the requested value as the only item on the stack.');
  lines.push('  It is graded on more inputs than the example shown, so it must work for any input of the kind');
  lines.push('  described.');
  if (definitionBudget === 0) {
    // Phase 5: what an inherited lexicon is worth when it is the only way to
    // name a sub-computation.
    lines.push('- In this round you may not define Words of your own. Write every solution with Core Words');
    lines.push(`  ${lexicon.entries.length > 0 ? 'and the inherited Words below' : 'only'}; leave \`definitions\` empty. Any definition you submit is ignored`);
    lines.push('  when grading, so a solution that calls one fails.');
  } else {
    lines.push(`- You may define your own Words. Every name must start with \`${agent}.\` (for example`);
    lines.push(`  \`${agent}.MEAN\`). Definitions made in this round may be offered to later agents.`);
    if (definitionBudget != null) {
      lines.push(`  Only the first ${definitionBudget} definitions in your list count; any after them are ignored when grading.`);
    }
  }
  lines.push('');

  if (lexicon.entries.length > 0) {
    lines.push('## Inherited Words');
    lines.push('These Words are already defined before your solution runs, and you may call them by name:');
    lines.push('');
    lines.push('| Name | Body | Note |');
    lines.push('| --- | --- | --- |');
    for (const e of lexicon.entries) lines.push(`| \`${e.name}\` | \`${e.body}\` | ${e.note || '—'} |`);
    lines.push('');
    if (harness || cli) {
      // Route B and the cli route: the server defines them. Pasting the definitions is how the
      // pilot's H5 counterexample spread — the expansion audited was not the
      // program that ran — so the harness route never asks for it.
      if (harness) {
        const names = lexicon.entries.map((e) => `"${e.name}"`).join(', ');
        lines.push('To use them, pass their names in the `words` argument of `compute` or `check`, for example');
        lines.push(`\`words: [${names}]\`. The server defines them before your source, and the result's \`prelude\``);
        lines.push('names each one it defined with its content digest.');
      } else {
        const names = lexicon.entries.map((e) => e.name).join(',');
        lines.push('To use them, name them with `--words` after the file, for example');
        lines.push(`\`${compute} ${work}try.ajisai --words ${names}\`.`);
        lines.push('The server defines them before your source; do not paste their definitions into your code.');
      }
    } else {
      lines.push('To test with them, put this text before your own code in a `compute` call:');
      lines.push('');
      lines.push('```');
      lines.push(prelude(lexicon.entries));
      lines.push('```');
    }
    lines.push('');
  }

  lines.push('## Tasks');
  for (const family of families) {
    lines.push('');
    lines.push(`### Input: ${family.inputKind}`);
    lines.push(`Example input: \`${family.inputs[0]}\``);
    lines.push('');
    for (const task of family.tasks) lines.push(`- \`${task.id}\`: ${task.prompt}`);
  }
  lines.push('');
  lines.push('## Submission');
  if (harness) {
    lines.push('Call the `submit` tool once, with your definitions (name, body, note) and one solution per task');
    lines.push('(task id, code that follows the input).');
  } else {
    lines.push(`Write one JSON file to \`${path}\` with this shape, then reply with the single word DONE:`);
    lines.push('');
    lines.push('```json');
    lines.push('{');
    lines.push(`  "agent": "${agent}", "generation": ${generation}, "condition": "${condition}",`);
    lines.push(definitionBudget === 0
      ? '  "definitions": [],'
      : `  "definitions": [ { "name": "${agent}.EXAMPLE", "body": "code inside the brackets", "note": "what it does, one line" } ],`);
    lines.push('  "solutions": { "<task id>": "code that follows the input" }');
    lines.push('}');
    lines.push('```');
  }
  lines.push('');
  if (definitionBudget !== 0) {
    lines.push('`body` is the code that goes between `[` and `]` in `[ body ] \'NAME\' DEF`. List definitions so that each');
    lines.push('comes after any of your own Words it calls. Solutions may call your Words and the inherited Words.');
  }
  if (callBudget === 0) return `${lines.join('\n')}\n`;
  lines.push(`Before ${harness ? 'submitting' : 'writing the file'}, run each solution through \`compute\` exactly as it will be graded: the`);
  lines.push(harness || cli
    ? `inherited Words (through \`${harness ? 'words' : '--words'}\`, if any), your definitions as DEF lines, the example input, the solution.`
    : 'inherited-Word text above (if any), your definitions as DEF lines, the example input, the solution.');
  return `${lines.join('\n')}\n`;
}
