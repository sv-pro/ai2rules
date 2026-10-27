# Add-ons

Defenses that sit around the ai2rules kernel, never inside it ([D96](../DECISIONS.md)).
Each is a separate package that installs on its own and combines with the others in
any mix. No kernel crate depends on one.

An add-on supplies a **fact**, never a verdict. The host maps the fact onto an input
the kernel already judges, such as a flow label (D95) or taint, and the world's
declared rule decides. Where a world declares no rule for the fact to meet, the host
may compose the add-on beside the kernel instead. In either case composition only
tightens: no add-on turns a refusal into an ALLOW or lowers a label.

First-party add-ons live here and reach the kernel the way a third-party one would,
through the host and the gate ABI.

| Add-on | Closes | Status |
|---|---|---|
| [`exfiltration/`](exfiltration/) | private values leaving through an argument, as is or lightly transformed (AI2-101) | experiment 1 of AI2-104 |
