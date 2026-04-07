# Architecture Review Criteria

Review the code changes for architectural consistency:

- **Layer boundaries**: Do changes respect the crate dependency graph? Domain types should not leak infrastructure concerns.
- **Separation of concerns**: Is each module focused on a single responsibility?
- **Error handling**: Are errors propagated correctly across layer boundaries?
- **Public API surface**: Are new public types and functions justified? Prefer `pub(crate)` where possible.
