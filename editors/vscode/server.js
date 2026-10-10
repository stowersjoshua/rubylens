// Client for `rubylens serve`: one long-running RubyLens process answers every view request, so
// opening a view doesn't start a process (see docs/spec/serve.md).
// Pure Node (no vscode import) so it can be tested with plain node.
//
// Requests and responses are JSON lines matched by id. A request the server didn't answer,
// because it couldn't start (a RubyLens without `serve`) or went away (crash, idle exit, its
// binary replaced), fails with `ServerGone`: the caller falls back to one-shot `rubylens gen`,
// and the next request starts a fresh server.
'use strict';

const cp = require('child_process');

class ServerGone extends Error {}

/** After this many servers in a row exit without answering anything, stop trying for this session. */
const MAX_FAILED_STARTS = 3;

class GenServer {
  /** @param {string} exe rubylens executable  @param {(line: string) => void} [log] */
  constructor(exe, log) {
    this.exe = exe;
    this.log = log || (() => {});
    this.proc = null;
    this.pending = new Map(); // id -> { resolve, reject }
    this.nextId = 1;
    this.failedStarts = 0;
    this.unavailable = null; // why serve can't be used, once we know
  }

  /** The `result` of a request; rejects with ServerGone when the server can't answer it. */
  request(params) {
    if (this.unavailable) return Promise.reject(new ServerGone(this.unavailable));
    if (!this.proc) this.start();
    const proc = this.proc;
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject, proc });
      proc.stdin.write(JSON.stringify({ id, ...params }) + '\n');
    });
  }

  start() {
    const proc = cp.spawn(this.exe, ['serve'], { stdio: ['pipe', 'pipe', 'pipe'] });
    proc.answered = 0;
    this.proc = proc;
    if (proc.pid) this.log(`rubylens serve started (${this.exe}, pid ${proc.pid})`);
    // Split stdout into lines in O(size): responses can be megabytes, arriving in 64 KB chunks.
    let parts = [];
    proc.stdout.setEncoding('utf8');
    proc.stdout.on('data', (chunk) => {
      let start = 0;
      let nl;
      while ((nl = chunk.indexOf('\n', start)) >= 0) {
        parts.push(chunk.slice(start, nl));
        this.onLine(proc, parts.join(''));
        parts = [];
        start = nl + 1;
      }
      if (start < chunk.length) parts.push(chunk.slice(start));
    });
    proc.stderr.setEncoding('utf8');
    proc.stderr.on('data', (d) => this.log(`rubylens serve: ${String(d).trimEnd()}`));
    proc.stdin.on('error', () => {}); // EPIPE when it has died; 'exit' handles the requests
    proc.on('error', (e) => this.gone(proc, e.message, e.code === 'ENOENT'));
    // 'close', not 'exit': it comes after the last of stdout has been read
    proc.on('close', (code, signal) => this.gone(proc, signal ? `killed by ${signal}` : `exited with code ${code}`, code === 2));
  }

  onLine(proc, line) {
    let msg;
    try {
      msg = JSON.parse(line);
    } catch (e) {
      this.log(`rubylens serve: unreadable response: ${e.message}`);
      return;
    }
    const p = this.pending.get(msg.id);
    if (!p) return;
    this.pending.delete(msg.id);
    proc.answered++;
    this.failedStarts = 0;
    if ('error' in msg) p.reject(new Error(msg.error));
    else p.resolve(msg.result);
  }

  /** The process `proc` went away (or never started): fail its unanswered requests. */
  gone(proc, why, unsupported) {
    if (proc.goneHandled) return;
    proc.goneHandled = true;
    if (this.proc === proc) this.proc = null;
    if (proc.answered === 0) {
      this.failedStarts++;
      // exit code 2 is a RubyLens that doesn't know `serve` (usage error); ENOENT, no RubyLens at all
      if (unsupported || this.failedStarts >= MAX_FAILED_STARTS) {
        this.unavailable = `rubylens serve is unavailable (${why}); using one rubylens process per view`;
        this.log(this.unavailable);
      }
    } else if (this.pending.size) {
      this.log(`rubylens serve ${why}; retrying its unanswered requests one by one`);
    }
    for (const [id, p] of this.pending) {
      if (p.proc !== proc) continue;
      this.pending.delete(id);
      p.reject(new ServerGone(why));
    }
  }

  /** Let the server finish what it's doing and exit. */
  stop() {
    if (this.proc) this.proc.stdin.end();
    this.proc = null;
  }
}

module.exports = { GenServer, ServerGone };
