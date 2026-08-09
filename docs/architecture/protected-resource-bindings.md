# Protected resource bindings

Conduit keeps a user's resource choice separate from authored meaning. A Form may require an operation whose offered implementation declares a named protected resource role, but the Form does not contain a filesystem path, browser object, descriptor, or base token.

Before planning, a base may supply a `ProtectedResourceGrant`. The grant names one operation role and carries an opaque handle plus exact host, boot, capability, resource class, access, byte bound, and commit policy. It is planning input, not authority inferred from availability. Raw locator material remains in the base's private handle table.

The planner admits a grant only when all of that scope matches a named `ResourceRequirement`. Every supplied grant must be consumed exactly once, a handle cannot fill two roles, and read/create/replace access must agree with its non-applicable/create-only/replace-existing commit policy. Missing, stale, ambiguous, incoherent, and unused grants fail before a Plan is returned.

The resulting `ProtectedResourceBinding` is sealed into the ordinary resource binding in the Plan. Its opaque handle, role, access, maximum byte count, and commit policy therefore contribute to fragment and Plan identity. Changing any of them after sealing invalidates verification. The binding conveys neither a raw path nor ambient permission; later Play start must still resolve it through the exact planned host and boot base boundary.

This contract does not implement copying or resource access. It establishes the planning seam required before a bounded copy implementation can cross the admitted host-operation boundary.
