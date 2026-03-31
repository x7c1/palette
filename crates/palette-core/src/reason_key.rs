/// Machine-readable error reason in `{namespace}/{value}` format.
///
/// Derive with `#[derive(palette_macros::ReasonKey)]` and
/// `#[reason_namespace = "..."]` on the enum.
pub trait ReasonKey {
    /// Error category (e.g. `"workflow_id"`, `"title"`).
    fn namespace(&self) -> &str;

    /// Specific error within the namespace (e.g. `"empty"`, `"too_long"`).
    fn value(&self) -> &str;

    /// Full reason key as `"{namespace}/{value}"`.
    fn reason_key(&self) -> String {
        format!("{}/{}", self.namespace(), self.value())
    }
}
