// A typo'd edit to books.cedar: one zero too many in the threshold. Payments
// between 10,001 and 100,000 now go through without the CFO. The property
// check must catch it with a concrete counterexample.

@id("agent-may-work")
permit (principal, action, resource);

@id("cfo-over-10k")
forbid (principal, action == Books::Action::"pay_invoice", resource)
when { resource.amount > 100000 && !context.cfo_approved };

@id("invoice-to-own-vendor")
forbid (principal, action == Books::Action::"email_vendor", resource)
unless { [resource].containsAll(context.carries_invoice_of) };
