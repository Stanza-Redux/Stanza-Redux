// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
//! Row-scoped, bounded cover loading. No native image decoder sees the full-size download.
use crate::*;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

type Cover = Option<Arc<Vec<u8>>>;
thread_local! {
    // Cache successes and misses for this session; never persist remote images in the library DB.
    static CACHE: RefCell<VecDeque<(String, Cover)>> = const { RefCell::new(VecDeque::new()) };
    static SLOTS: RefCell<Slots> = const { RefCell::new(Slots { free: 4, queue: VecDeque::new() }) };
}
/// At most four cover requests at once, granted strictly in request order. A native list may
/// bind far more rows than it shows (GtkListView binds a whole 120-book page), so the queue
/// can be long. A semaphore whose woken waiter can lose the freed slot to a newcomer, and
/// rejoin at the back, starves the first rows behind all the others.
struct Slots {
    free: usize,
    queue: VecDeque<Rc<RefCell<Turn>>>,
}
#[derive(Default)]
struct Turn {
    granted: bool,
    waker: Option<std::task::Waker>,
}
/// A held request slot; dropping it hands the slot to the next waiter.
struct Slot;
impl Drop for Slot {
    fn drop(&mut self) {
        release();
    }
}
fn release() {
    // Wake after the borrow ends: a waker may poll its task right away.
    let waker = SLOTS.with(|s| {
        let mut s = s.borrow_mut();
        // The queue's reference is the only one left once a waiter's request was cancelled.
        while let Some(turn) = s.queue.pop_front() {
            if Rc::strong_count(&turn) > 1 {
                let mut turn = turn.borrow_mut();
                turn.granted = true;
                return turn.waker.take();
            }
        }
        s.free += 1;
        None
    });
    if let Some(waker) = waker {
        waker.wake();
    }
}
async fn slot() -> Slot {
    let turn = SLOTS.with(|s| {
        let mut s = s.borrow_mut();
        if s.free > 0 {
            s.free -= 1;
            return None;
        }
        let turn = Rc::new(RefCell::new(Turn::default()));
        s.queue.push_back(turn.clone());
        Some(turn)
    });
    if let Some(turn) = turn {
        Wait(Some(turn)).await;
    }
    Slot
}
/// Waits for a granted turn. Dropped after the grant but before it was taken, it passes the
/// slot on; dropped before, it simply leaves the queue.
struct Wait(Option<Rc<RefCell<Turn>>>);
impl std::future::Future for Wait {
    type Output = ();
    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<()> {
        let Some(turn) = &self.0 else {
            return std::task::Poll::Ready(());
        };
        if turn.borrow().granted {
            self.0 = None;
            return std::task::Poll::Ready(());
        }
        turn.borrow_mut().waker = Some(cx.waker().clone());
        std::task::Poll::Pending
    }
}
impl Drop for Wait {
    fn drop(&mut self) {
        if self.0.take().is_some_and(|turn| turn.borrow().granted) {
            release();
        }
    }
}
fn cached(url: &str) -> Option<Cover> {
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        let i = c.iter().position(|(u, _)| u == url)?;
        let entry = c.remove(i)?;
        let result = entry.1.clone();
        c.push_back(entry);
        Some(result)
    })
}
async fn load(url: String) -> Cover {
    if let Some(bytes) = cached(&url) {
        return bytes;
    }
    let _slot = slot().await;
    if let Some(bytes) = cached(&url) {
        return bytes;
    }
    let result = async {
        let bytes = if let Some(path) = url.strip_prefix("library:") {
            day_part_fs::read_future(path).await.ok()?
        } else {
            fetch_bytes(&url, 4 * 1024 * 1024, None).await.ok()?.0
        };
        #[cfg(not(target_arch = "wasm32"))]
        let bytes = background(move || thumbnail(bytes)).await.ok()?.ok()?;
        Some(Arc::new(bytes))
    }
    .await;
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        c.push_back((url, result.clone()));
        while c.len() > 64
            || c.iter()
                .filter_map(|(_, b)| b.as_ref())
                .map(|b| b.len())
                .sum::<usize>()
                > 16 * 1024 * 1024
        {
            c.pop_front();
        }
    });
    result
}
/// Native parsing and raster work run off the UI thread. Web uses the browser's async
/// image decoder; small, size-limited OPDS documents are parsed on its single JS thread.
pub async fn background<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (send, receive) = futures_channel::oneshot::channel();
        std::thread::Builder::new()
            .name("stanza-decode".into())
            .spawn(move || {
                let _ = send.send(work());
            })
            .map_err(|e| e.to_string())?;
        receive.await.map_err(|e| e.to_string())
    }
    #[cfg(target_arch = "wasm32")]
    {
        Ok(work())
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn thumbnail(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    use std::io::Cursor;
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|e| e.to_string())?;
    let small = decoded.thumbnail(320, 440);
    let mut out = Cursor::new(Vec::new());
    small
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}
/// Keep one image view and one fixed frame throughout loading and cell reuse. The
/// requested URL is checked while rendering as well as on completion: bindings may
/// run before the URL watcher when a native cell is rebound to a different book.
pub fn view(url: impl Fn() -> String + 'static, width: f64, height: f64) -> impl Piece {
    image_view(url, width, height, || {
        day::ImageSource::from(res::vectors::book_placeholder)
    })
}

/// Favicons use the same bounded, cancellable cache as covers, with a square fallback.
pub fn favicon(url: impl Fn() -> String + 'static) -> impl Piece {
    let url = Rc::new(url);
    let fallback_url = url.clone();
    image_view(
        move || url(),
        32.,
        32.,
        move || {
            // Original source-specific artwork also works offline and when web CORS denies icons.
            // Project Gutenberg's OPDS 2 feed is served from pglaf.org, its foundation's domain.
            let fallback = match opds::catalog_host(&fallback_url()).as_str() {
                "gutenberg.org" | "www.gutenberg.org" => res::vectors::catalog_gutenberg,
                h if h == "pglaf.org" || h.ends_with(".pglaf.org") => {
                    res::vectors::catalog_gutenberg
                }
                "standardebooks.org" | "www.standardebooks.org" => res::vectors::catalog_standard,
                "ebooksgratuits.com" | "www.ebooksgratuits.com" => res::vectors::catalog_gratuits,
                _ => res::vectors::catalog_source,
            };
            day::ImageSource::from(fallback)
        },
    )
    .padding(10.)
    .background(Color::hex(0xeee8da))
    .corner_radius(14.)
}
fn image_view(
    url: impl Fn() -> String + 'static,
    width: f64,
    height: f64,
    fallback: impl Fn() -> day::ImageSource + 'static,
) -> impl Piece {
    let url = Rc::new(url);
    let shown = Signal::new((String::new(), None::<Arc<Vec<u8>>>));
    let generation = Rc::new(std::cell::Cell::new(0_u64));
    let task = Rc::new(RefCell::new(None::<day::TaskHandle>));
    let start = {
        let task = task.clone();
        let generation = generation.clone();
        move |url: String| {
            let request = generation.get().wrapping_add(1);
            generation.set(request);
            if let Some(old) = task.borrow_mut().take() {
                old.abort();
            }
            // Publish a cache hit synchronously; an uncached or missing cover shows the
            // same book-shaped skeleton, never the previous occupant's pixels.
            let hit = cached(&url);
            shown.set((url.clone(), hit.clone().flatten()));
            if !url.is_empty() && hit.is_none() {
                let generation = generation.clone();
                *task.borrow_mut() = Some(day::task(async move {
                    let bytes = load(url.clone()).await;
                    if generation.get() == request {
                        shown.set((url, bytes));
                    }
                }));
            }
        }
    };
    start(url());
    let watched_url = url.clone();
    // A list rebind can hand the row the URL it already shows or is loading. Keep that load:
    // restarting drops the cover and queues the request behind every other row's.
    watch(
        move || watched_url(),
        move |url, previous| {
            if previous != Some(url) {
                start(url.clone())
            }
        },
    );
    Scope::current().on_cleanup(move || {
        generation.set(generation.get().wrapping_add(1));
        if let Some(task) = task.borrow_mut().take() {
            task.abort();
        }
    });
    let source_url = url.clone();
    image(move || cover_source(&source_url(), &shown.get(), &fallback()))
        .fit()
        .decorative()
        .id_of(move || {
            let requested = url();
            let (loaded, bytes) = shown.get();
            let state = if requested == loaded && bytes.is_some() {
                "cover"
            } else {
                "placeholder"
            };
            format!("catalog-{state}-{width}-{requested}")
        })
        .frame(width, height)
}
fn cover_source(
    requested: &str,
    loaded: &(String, Cover),
    fallback: &day::ImageSource,
) -> day::ImageSource {
    if loaded.0 == requested {
        if let Some(bytes) = &loaded.1 {
            return day::ImageSource::from(bytes.clone());
        }
    }
    fallback.clone()
}
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    #[test]
    fn request_slots_are_granted_in_order_and_cancelled_turns_pass_them_on() {
        use std::future::Future;
        type Pending = std::pin::Pin<Box<dyn Future<Output = Slot>>>;
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        let mut poll = |f: &mut Pending| match f.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(slot) => Some(slot),
            std::task::Poll::Pending => None,
        };
        let mut held = Vec::new();
        for _ in 0..4 {
            let mut f: Pending = Box::pin(slot());
            held.push(poll(&mut f).expect("a free slot is granted at once"));
        }
        let mut first: Pending = Box::pin(slot());
        let mut second: Pending = Box::pin(slot());
        let mut third: Pending = Box::pin(slot());
        assert!(poll(&mut first).is_none() && poll(&mut second).is_none());
        assert!(poll(&mut third).is_none());
        // The freed slot goes to the oldest waiter, never to a newer one.
        held.pop();
        assert!(poll(&mut third).is_none() && poll(&mut second).is_none());
        let first_slot = poll(&mut first).expect("the oldest waiter is granted");
        // A waiter cancelled after its grant hands the slot on to the next one.
        held.pop();
        drop(second);
        assert!(poll(&mut third).is_some());
        drop((first_slot, held));
        SLOTS.with(|s| assert_eq!(s.borrow().free, 4));
    }
    #[test]
    fn recycled_rows_never_show_a_cover_for_another_url() {
        let bytes = Arc::new(vec![1, 2, 3]);
        let loaded = ("book-a".into(), Some(bytes.clone()));
        let placeholder = day::ImageSource::from(res::vectors::book_placeholder);
        assert_eq!(
            cover_source("book-a", &loaded, &placeholder),
            day::ImageSource::from(bytes)
        );
        assert_eq!(cover_source("book-b", &loaded, &placeholder), placeholder);
        assert_eq!(cover_source("", &loaded, &placeholder), placeholder);
        assert_eq!(
            cover_source("book-a", &("book-a".into(), None), &placeholder),
            placeholder
        );
    }
    #[test]
    fn thumbnails_are_bounded_and_invalid_images_fail() {
        let image = image::DynamicImage::new_rgb8(1000, 1500);
        let mut encoded = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        let bytes = thumbnail(encoded.into_inner()).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert!(decoded.width() <= 320 && decoded.height() <= 440);
        assert!(thumbnail(b"not an image".to_vec()).is_err());
        let icon = image::DynamicImage::new_rgba8(32, 32);
        let mut encoded = std::io::Cursor::new(Vec::new());
        icon.write_to(&mut encoded, image::ImageFormat::Ico)
            .unwrap();
        assert!(thumbnail(encoded.into_inner()).is_ok());
    }
}
