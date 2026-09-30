// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
//! Row-scoped, bounded cover loading. No native image decoder sees the full-size download.
use crate::*;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

static REQUESTS: std::sync::LazyLock<async_lock::Semaphore> =
    std::sync::LazyLock::new(|| async_lock::Semaphore::new(4));
type Cover = Option<Arc<Vec<u8>>>;
thread_local! {
    // Cache successes and misses for this session; never persist remote images in the library DB.
    static CACHE: RefCell<VecDeque<(String, Cover)>> = const { RefCell::new(VecDeque::new()) };
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
    let _permit = REQUESTS.acquire().await;
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
    watch(move || watched_url(), move |url, _| start(url.clone()));
    Scope::current().on_cleanup(move || {
        generation.set(generation.get().wrapping_add(1));
        if let Some(task) = task.borrow_mut().take() {
            task.abort();
        }
    });
    let source_url = url.clone();
    image(move || cover_source(&source_url(), &shown.get()))
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
fn cover_source(requested: &str, loaded: &(String, Cover)) -> day::ImageSource {
    if loaded.0 == requested {
        if let Some(bytes) = &loaded.1 {
            return day::ImageSource::from(bytes.clone());
        }
    }
    day::ImageSource::from(res::vectors::book_placeholder)
}
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    #[test]
    fn recycled_rows_never_show_a_cover_for_another_url() {
        let bytes = Arc::new(vec![1, 2, 3]);
        let loaded = ("book-a".into(), Some(bytes.clone()));
        assert_eq!(
            cover_source("book-a", &loaded),
            day::ImageSource::from(bytes)
        );
        let placeholder = day::ImageSource::from(res::vectors::book_placeholder);
        assert_eq!(cover_source("book-b", &loaded), placeholder);
        assert_eq!(cover_source("", &loaded), placeholder);
        assert_eq!(
            cover_source("book-a", &("book-a".into(), None)),
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
    }
}
