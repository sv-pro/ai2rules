// The WebMCP stand's business rules in Cedar (AI2-111 spike). Same content as
// ../profile.yaml, written as policies instead of classes.

// The tool layer's binding, as the inventory found it: each tool lives on one
// system. Cedar needs it to say "back to the system the data came from".
@id("crm-tools")
permit (
  principal,
  action in [Northwind::Action::"get_customer", Northwind::Action::"list_customers", Northwind::Action::"add_note"],
  resource == Northwind::System::"crm"
);

@id("shop-tools")
permit (
  principal,
  action in [Northwind::Action::"search_products", Northwind::Action::"add_to_cart", Northwind::Action::"submit_order", Northwind::Action::"post_review"],
  resource == Northwind::System::"shop"
);

// Customer records are private: data read from a confidential system may go
// back to that system and nowhere else, through any tool, read or write.
@id("private-stays-home")
forbid (principal, action, resource)
unless { [resource].containsAll(context.private_from) };

// An order spends money: a person approves it.
@id("money-needs-approval")
forbid (principal, action == Northwind::Action::"submit_order", resource)
unless { context.approved };

// No publishing in the company's name.
@id("no-public-posting")
forbid (principal, action == Northwind::Action::"post_review", resource);

// The agent does not write to customer records.
@id("no-record-writes")
forbid (principal, action == Northwind::Action::"add_note", resource);
