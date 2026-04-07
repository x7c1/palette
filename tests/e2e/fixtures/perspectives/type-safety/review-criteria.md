# Type Safety Review Criteria

Review the code changes for type-level correctness:

- **Exhaustive matching**: Are all enum variants handled? Avoid wildcard `_` patterns on domain enums.
- **Value objects**: Are domain values validated at construction? Reject invalid states at parse time.
- **Option vs Result**: Is `None` a valid state or an error? Use `Result` when absence is unexpected.
- **Clone boundaries**: Are `Clone` derives justified? Prefer borrowing where ownership transfer is unnecessary.
