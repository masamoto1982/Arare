// A thin MCP client for the Ajisai server in tools/mcp-server. The experiment
// talks to the language only through the same `compute` tool its subject
// agents use, so the grader and the subjects see one engine.

import { Client } from '../../mcp-server/node_modules/@modelcontextprotocol/sdk/dist/esm/client/index.js';
import { StdioClientTransport } from '../../mcp-server/node_modules/@modelcontextprotocol/sdk/dist/esm/client/stdio.js';
import { fileURLToPath } from 'node:url';

const SERVER = fileURLToPath(new URL('../../mcp-server/index.js', import.meta.url));

/**
 * `phrasebook`, when given, is a file the server serves for the `words`
 * argument in place of the repository's phrasebook (`AJISAI_PHRASEBOOK`):
 * route B hands a generation its inherited lexicon this way.
 */
export async function connect({ phrasebook } = {}) {
  const env = phrasebook ? { ...process.env, AJISAI_PHRASEBOOK: phrasebook } : undefined;
  const transport = new StdioClientTransport({ command: 'node', args: [SERVER], ...(env ? { env } : {}) });
  const client = new Client({ name: 'lexicon-emergence', version: '0.1.0' });
  await client.connect(transport);

  /** The tool's text reply, exactly as a subject agent would read it. */
  async function callText(name, args) {
    const result = await client.callTool({ name, arguments: args });
    return result.content.map((c) => c.text ?? '').join('\n');
  }
  const call = async (name, args) => JSON.parse(await callText(name, args));

  return {
    compute: (source, words) => call('compute', words ? { source, words } : { source }),
    check: (source, words) => call('check', words ? { source, words } : { source }),
    callText,
    /** The server's own definitions of the named tools, in the order given. */
    async tools(names) {
      const { tools } = await client.listTools();
      return names.map((name) => {
        const tool = tools.find((t) => t.name === name);
        if (!tool) throw new Error(`the Ajisai MCP server has no tool ${name}`);
        return tool;
      });
    },
    close: () => client.close(),
  };
}

/** The final stack as display strings, bottom to top; null when the run did not end ok. */
export function stackOf(result) {
  return result.status === 'ok' ? (result.stackDisplay ?? []) : null;
}
