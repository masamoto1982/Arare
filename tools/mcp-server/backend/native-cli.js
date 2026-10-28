// Native-process backend: shells out to a built `ajisai` binary per call, the
// same one-process-per-call boundary the CLI's own `agent` command uses.
//
// The program travels on the child's stdin (`ajisai agent <op> -`). Writing it
// to a temporary file first — which this backend used to do, synchronously —
// needed a writable temporary directory, blocked the stdio server's event loop
// three times per call, and left the program on disk for the duration; the
// pipe needs none of that and the CLI reads it identically.
//
// Every method resolves to the parsed JSON envelope on success —
// including a language-level `NIL` or `ERROR` result, which is still a
// *successful* MCP tool call. A thrown `HostError` means a host failure
// (timeout, spawn failure, an oversized or non-JSON response), never a
// translated Ajisai `ERROR`.

import { execFile } from "node:child_process";
import { HostError } from "../host-error.js";

// A timed-out child that ignores SIGTERM would keep the slot the execution
// gate already counts as released, so the deadline kills outright.
const KILL_SIGNAL = "SIGKILL";

export class NativeCliBackend {
  static kind = "nativeCli";

  constructor({ bin, wallTimeMs, responseBytes, executionSteps }) {
    this.bin = bin;
    this.wallTimeMs = wallTimeMs;
    this.responseBytes = responseBytes;
    this.executionSteps = executionSteps;
  }

  get kind() {
    return NativeCliBackend.kind;
  }

  #exec(args, source) {
    return new Promise((resolve, reject) => {
      const child = execFile(
        this.bin,
        args,
        {
          encoding: "utf8",
          timeout: this.wallTimeMs,
          killSignal: KILL_SIGNAL,
          maxBuffer: this.responseBytes,
        },
        (error, stdout) => {
          if (!error) return resolve(stdout);
          // Only the deadline's own kill is a timeout: `killed` is set when
          // node sent the signal, and the deadline sends SIGKILL. A child that
          // dies of any other signal (a crash, an abort) crashed; retrying it
          // would crash again, and no wall time was exceeded.
          if (error.killed && error.signal === KILL_SIGNAL) {
            return reject(
              new HostError("timeout", `Execution exceeded the ${this.wallTimeMs} ms wall-time limit.`, {
                detail: `signal=${error.signal}`,
              }),
            );
          }
          if (error.signal) {
            return reject(
              new HostError("backendFailure", `The Ajisai backend was terminated by ${error.signal}.`, {
                detail: `bin=${this.bin} signal=${error.signal}`,
              }),
            );
          }
          if (error.code === "ERR_CHILD_PROCESS_STDIO_MAXBUFFER") {
            return reject(
              new HostError(
                "responseTooLarge",
                `The result exceeds the ${this.responseBytes}-byte response limit. Reduce the size of the value left on the stack.`,
              ),
            );
          }
          // Ajisai language errors intentionally exit 1 with a JSON report
          // on stdout — a *successful* MCP call carrying a structured error.
          if (error.code === 1 && stdout) return resolve(stdout);
          reject(
            new HostError("backendFailure", "The Ajisai backend failed to produce a result.", {
              detail: `bin=${this.bin} code=${error.code} message=${error.message}`,
            }),
          );
        },
      );
      child.on("error", (error) => {
        reject(
          new HostError("backendFailure", "The Ajisai backend could not be started.", {
            detail: `bin=${this.bin} message=${error.message}`,
          }),
        );
      });
      child.stdin.on("error", () => {
        // The child exited before reading its input; the exec callback above
        // is the one that reports why.
      });
      // Written through byte-for-byte. The temporary-file form used to append
      // a trailing newline, which silently made the program one byte longer
      // than the caller's — enough, at exactly the source-byte ceiling, for
      // this backend to reject a program the other one accepted.
      child.stdin.end(source, "utf8");
    });
  }

  async #runAgent(source, operation, extraArgs) {
    // `--compact`: the envelope as the WASM entry points return it, byte for
    // byte. The CLI's default is indented for a reader, about twice the bytes
    // for a large stack, so the same result used to pass the `responseBytes`
    // ceiling on the WASM backend and fail it here (a 7,000-element MAP:
    // 608 KB compact against over 1 MB indented). Measuring the same bytes is
    // what makes the ceiling one limit rather than two.
    const stdout = await this.#exec(["agent", operation, "-", "--compact", ...extraArgs], source);
    // The child's own `maxBuffer` bounds what it may write, but a backend must
    // not depend on a limit only one of the two implementations enforces.
    if (Buffer.byteLength(stdout, "utf8") > this.responseBytes) {
      throw new HostError(
        "responseTooLarge",
        `The result exceeds the ${this.responseBytes}-byte response limit. Reduce the size of the value left on the stack.`,
      );
    }
    try {
      return JSON.parse(stdout);
    } catch {
      throw new HostError("malformedBackendResponse", "The Ajisai backend returned a non-JSON response.", {
        detail: `first bytes: ${stdout.slice(0, 200)}`,
      });
    }
  }

  compute(source) {
    return this.#runAgent(source, "compute", ["--step-limit", String(this.executionSteps)]);
  }

  check(source) {
    return this.#runAgent(source, "check", []);
  }

  inferContracts(source) {
    return this.#runAgent(source, "infer-contracts", []);
  }
}
