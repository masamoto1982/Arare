// The host-failure half of the MCP boundary.
//
// Ajisai's own outcomes — a value, a reason-carrying `NIL`, a language `ERROR`
// — are all *successful* tool calls carrying a structured report. What was
// left unstructured was the other half: a missing backend, an exhausted
// budget, a timeout, a malformed backend response. Those arrived as prose, so
// a caller wanting to tell "retry in a moment" from "this will never work"
// had to match on English substrings — which the server's own self-test was
// already doing, in the one place a brittle dependency is most visible.
//
// Every host failure now carries a stable `code`, a `retryable` flag, and a
// message written for a model rather than for an operator. Internal detail —
// absolute paths, environment-variable names, spawn diagnostics — stays on
// the server's stderr, where the person who can act on it is looking.

/** Machine-readable host-failure codes and what a caller should do with them. */
export const HOST_ERRORS = Object.freeze({
  /** The request itself is malformed: a missing or non-string `source`. */
  invalidRequest: { retryable: false },
  /** No such tool. */
  unknownTool: { retryable: false },
  /** `source` is larger than the declared `sourceBytes` limit. */
  sourceTooLarge: { retryable: false, limit: "sourceBytes" },
  /** Neither a native binary nor the packaged WASM bundle is usable. */
  backendUnavailable: { retryable: false },
  /** The declared `concurrentExecutions` ceiling is saturated. */
  capacityExhausted: { retryable: true, limit: "concurrentExecutions" },
  /** Execution passed the declared `wallTimeMs` ceiling and was killed. */
  timeout: { retryable: true, limit: "wallTimeMs" },
  /** The backend's response passed the declared `responseBytes` ceiling. */
  responseTooLarge: { retryable: false, limit: "responseBytes" },
  /** The backend answered with something that is not the result envelope. */
  malformedBackendResponse: { retryable: false },
  /** The backend failed for a reason none of the above names. */
  backendFailure: { retryable: false },
  /** The packaged Word registry is missing or does not match its metadata. */
  registryUnavailable: { retryable: false },
});

/**
 * A failure of the *host*, never of the Ajisai program.
 *
 * `message` is model-facing and must stay free of host detail. `detail` is
 * the operator-facing half and never crosses the MCP boundary.
 */
export class HostError extends Error {
  constructor(code, message, { detail, retryAfterMs } = {}) {
    super(message);
    this.name = "HostError";
    if (!Object.hasOwn(HOST_ERRORS, code)) {
      throw new Error(`unknown host-error code: ${code}`);
    }
    this.code = code;
    this.detail = detail;
    this.retryAfterMs = retryAfterMs;
  }

  get retryable() {
    return HOST_ERRORS[this.code].retryable;
  }

  /** The declared limit this failure is about, when it is about one. */
  get limit() {
    return HOST_ERRORS[this.code].limit;
  }

  /**
   * Wrap anything a backend threw. A `HostError` passes through unchanged; a
   * bare `Error` becomes `backendFailure` with its message demoted to
   * server-side detail, because a spawn failure's message carries the binary's
   * absolute path and a module-load failure carries the install layout.
   */
  static from(error) {
    if (error instanceof HostError) return error;
    return new HostError("backendFailure", "The Ajisai backend failed to produce a result.", {
      detail: error?.message ?? String(error),
    });
  }
}

/**
 * Log the operator-facing half. stderr is the only channel a stdio MCP server
 * has that is not the protocol itself.
 */
export function logHostError(error, context) {
  const detail = error.detail ? ` detail=${error.detail}` : "";
  console.error(`[ajisai-mcp] ${context} code=${error.code}${detail}`);
}
