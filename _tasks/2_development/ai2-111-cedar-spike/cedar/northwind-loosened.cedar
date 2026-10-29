// A plausible edit to northwind.cedar that loosens it: "search is only a read,
// let it through". This is exactly the WebMCP lab's S5b leak (the account code
// goes out inside a search query). The regression check must catch it.

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

@id("private-stays-home")
forbid (principal, action, resource)
unless {
  [resource].containsAll(context.private_from) ||
  action == Northwind::Action::"search_products"
};

@id("money-needs-approval")
forbid (principal, action == Northwind::Action::"submit_order", resource)
unless { context.approved };

@id("no-public-posting")
forbid (principal, action == Northwind::Action::"post_review", resource);

@id("no-record-writes")
forbid (principal, action == Northwind::Action::"add_note", resource);
