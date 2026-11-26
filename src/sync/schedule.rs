// SPDX-License-Identifier: GPL-2.0-or-later
//! Coalesce local reading edits without waiting for bulk EPUB transfers.
use super::*;

#[derive(Default)]
struct Requests {
    running: bool,
    pending: bool,
}
impl Requests {
    fn request(&mut self) -> bool {
        self.pending = true;
        !std::mem::replace(&mut self.running, true)
    }
    fn begin(&mut self) {
        self.pending = false;
    }
    fn again(&mut self) -> bool {
        if self.pending {
            true
        } else {
            self.running = false;
            false
        }
    }
}
thread_local! { static REQUESTS: RefCell<Requests> = RefCell::default(); }

fn allowed() -> bool {
    setting().get_untracked().is_some()
        && !matches!(status().get_untracked(), Phase::Failed(failure) if failure.pauses_retry())
        && RETRY.with(|retry| retry.get().is_none_or(|at| std::time::Instant::now() >= at))
}
async fn exchange() -> Attempt {
    let local = |error: Error| Failure::sync(Stage::Sync, &error);
    let Some(account) = setting().get_untracked() else {
        return Ok(());
    };
    let db = library().map_err(local)?;
    let replica = replica(&db, &account).await.map_err(local)?;
    let dav = store(&account)?;
    descriptor(&dav, Some(&account.identity))
        .await
        .map_err(local)?;
    metadata(&replica, &dav).await.map_err(local)?;
    db.check_external().map_err(|error| local(error.into()))?;
    Ok(())
}
fn report(result: Attempt) {
    // A metadata-only success does not claim that outstanding EPUB transfers finished.
    if let Err(error) = result
        && status().get_untracked() != Phase::Working
    {
        finish(Err(error));
    }
}
fn request(mut delay: u32) {
    if !allowed() || !REQUESTS.with(|state| state.borrow_mut().request()) {
        return;
    }
    day::task(async move {
        loop {
            if delay != 0 {
                day::sleep(delay).await;
            }
            REQUESTS.with(|state| state.borrow_mut().begin());
            if allowed() {
                report(exchange().await);
            }
            if !REQUESTS.with(|state| state.borrow_mut().again()) {
                break;
            }
            delay = 1_000;
        }
    });
}
pub(crate) fn changed() {
    request(1_000);
}
pub(crate) fn flush() {
    request(0);
}
pub(crate) async fn before_open() {
    if !allowed() {
        return;
    }
    // Offline reading must remain available even when a server cannot be reached.
    let result = futures_lite::future::race(async { Some(exchange().await) }, async {
        day::sleep(5_000).await;
        None
    })
    .await;
    if let Some(result) = result {
        report(result);
    }
}
pub(super) fn start() {
    on_lifecycle(Lifecycle::DidBecomeActive, flush);
    on_lifecycle(Lifecycle::WillResignActive, flush);
    day::task(async {
        loop {
            day::sleep(10_000).await;
            flush();
        }
    });
}
#[cfg(test)]
mod tests {
    use super::Requests;
    #[test]
    fn reading_edits_coalesce_and_edits_during_exchange_get_another_pass() {
        let mut queue = Requests::default();
        assert!(queue.request());
        assert!(!queue.request());
        queue.begin();
        assert!(!queue.request());
        assert!(queue.again());
        queue.begin();
        assert!(!queue.again());
        assert!(queue.request());
    }
}
