// JSONL trace (CONTRACTS.md §4). One file per cell. Records carry a logical
// `seq`, never wall-clock time, so two runs of the same cell produce
// byte-identical traces.
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';

export class Trace {
  // `onRecord` sees each record as it is written (the live demo streams them);
  // it never changes what is recorded.
  constructor(cell, { onRecord } = {}) {
    this.cell = cell;
    this.records = [];
    this.onRecord = onRecord;
  }

  write(kind, fields = {}) {
    const rec = { seq: this.records.length + 1, cell: this.cell, kind, ...fields };
    this.records.push(rec);
    this.onRecord?.(rec);
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
