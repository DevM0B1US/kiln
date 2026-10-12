use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::{ToolError, ToolResult};

/// Cooperative cancellation, checked between items.
#[derive(Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// A token with nothing cancelled yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Ask the running job to stop after the current item.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Whether cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    /// Whether two tokens share the same underlying flag. Clones are the same
    /// token; separately constructed ones never are.
    #[must_use]
    pub fn same_flag(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// `Err(Cancelled)` once [`Self::cancel`] has been called.
    ///
    /// # Errors
    ///
    /// Returns [`ToolError::Cancelled`] when cancellation was requested.
    pub fn check(&self) -> ToolResult<()> {
        if self.is_cancelled() {
            Err(ToolError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_takes_effect() {
        let token = CancelToken::new();
        assert!(token.check().is_ok());

        token.cancel();
        assert!(token.is_cancelled());
        assert!(matches!(token.check(), Err(ToolError::Cancelled)));
    }

    #[test]
    fn clones_share_one_flag() {
        let token = CancelToken::new();
        let clone = token.clone();

        clone.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn separate_tokens_are_distinct() {
        let token = CancelToken::new();

        assert!(!token.same_flag(&CancelToken::new()));
        assert!(token.same_flag(&token.clone()));
    }
}
