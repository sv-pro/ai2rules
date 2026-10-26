// JSONL trace (CONTRACTS.md §4). One file per cell. Records carry a logical
// `seq`, never wall-clock time, so two runs of the same cell produce
// byte-identical traces.
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

export class Trace {
  constructor(cell) {
    this.cell = cell;
    this.records = [];
  }

  write(kind, fields = {}) {
    const rec = { seq: this.records.length + 1, cell: this.cell, kind, ...fields };
    this.records.push(rec);
    return rec;
  }

  ofKind(kind) {
    return this.records.filter((r) => r.kind === kind);
  }

  save(file) {
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, this.records.map((r) => JSON.stringify(r)).join('\n') + '\n');
  }
}
