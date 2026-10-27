# Exfiltration add-on

Finds private values in a call's outbound arguments and reports them (AI2-101). It
reports facts, never a verdict (D96). No model, no I/O, no dependencies: the same
observations and arguments always give the same findings.

```js
import { createExfiltrationGuard } from './index.js';

const guard = createExfiltrationGuard();
guard.observe(recordFromTheCrm, { source: 'crm/get_customer' }); // the host says: this is private
guard.inspect({ query: 'acct-7q4m…' });
// → [{ addon: 'exfiltration', argument: 'query', form: 'case_folded', source: 'crm/get_customer' }]
```

**What counts as private is the host's word.** The add-on never decides it. In the
WebMCP lab, the host passes it whatever the task's sensitive origins return.

**Fingerprints.**
- every string in an observed value at least 8 characters long, kept whole;
- within it, each token of that length that carries a digit or a symbol, as
  identifiers, codes, emails, phone numbers and dates do.

Plain words are not fingerprints, so ordinary text that shares a word with a record
is not a finding.

**Forms recognised.**
- as is;
- case-folded;
- with separators removed;
- reversed;
- base64-encoded;
- percent-encoded.

A paraphrase, a summary, a hash or any other transform is not recognised.

**Where its findings go.** The host maps each finding. In a world that declares flow
sinks, a finding raises the argument's label to Private and the kernel's flow
contract decides. In a world with no sinks, the host refuses the call beside the
kernel. Experiment 1 of AI2-104 measures both carriers in the WebMCP lab:
[`labs/webmcp/results/experiments/exfiltration/`](../../labs/webmcp/results/experiments/exfiltration/).

```bash
node --test test/*.test.js
```
