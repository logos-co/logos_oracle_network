use std::future::Future;

/// Run a LEZ call (wallet / contract client) so it can be used inside a spawned task
///
/// Why: with LEZ v0.2.4, some `WalletCore` methods (`get_account_public`, `get_accounts_nonces`, used
/// by every spel generated contract client) return futures that rustc cannot prove `Send`
/// (they use an async closure taking a `&SequencerClient`, see rust-lang/rust#110338).
/// As a result, any task awaiting a contract client call cannot be passed to `tokio::spawn`
/// (or `JoinSet::spawn`) - the compiler error is: "implementation of `Send` is not general enough".
///
/// How: the future is created and awaited on a tokio blocking thread (`spawn_blocking` +
/// `Handle::block_on`), so it never needs to be `Send`. Only the closure creating it (and its
/// captured values) must be `Send + 'static`, so pass owned (cloned) values to the closure.
///
/// Note: each call uses a thread from the tokio blocking pool for its whole duration;
///       fine for contract calls (a few per round) but not for a hot path.
///
/// Example:
/// ```ignore
/// let rc_info = rc_info.clone();
/// let state = run_lez(move || async move { fetch_state(&rc_info).await }).await?;
/// ```
pub async fn run_lez<F, Fut, T>(f: F) -> anyhow::Result<T>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = anyhow::Result<T>>,
    T: Send + 'static,
{
    let handle = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || handle.block_on(f())).await?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_run_lez_non_send_future_in_spawned_task() {
        let value = 42u64;
        // Without run_lez, this would not compile: the future holds an Rc (not Send) across an await
        let res = tokio::spawn(async move {
            run_lez(move || async move {
                let rc = Rc::new(value);
                tokio::task::yield_now().await;
                Ok(*rc + 1)
            }).await
        }).await.unwrap().unwrap();
        assert_eq!(res, 43);
    }
}
