// Accounts payable rules (AI2-111 spike).

@id("agent-may-work")
permit (principal, action, resource);

// Over 10,000 the CFO approves.
@id("cfo-over-10k")
forbid (principal, action == Books::Action::"pay_invoice", resource)
when { resource.amount > 10000 && !context.cfo_approved };

// Invoice data goes to its own vendor and nowhere else.
@id("invoice-to-own-vendor")
forbid (principal, action == Books::Action::"email_vendor", resource)
unless { [resource].containsAll(context.carries_invoice_of) };
