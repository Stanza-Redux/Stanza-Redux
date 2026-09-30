// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
//! One import path for the picker and operating-system document activation.
use crate::*;

fn separate_reader() -> bool {
    !cfg!(any(
        target_os = "ios",
        target_os = "android",
        target_env = "ohos",
        target_arch = "wasm32"
    ))
}
pub fn register(a: App) {
    day::on_open_files(move |files| {
        day::task(async move {
            for path in files {
                import(a, FileUrl::new(path)).await;
            }
        });
    });
}
pub fn open_command(a: App) -> CommandHandle {
    Command {
        id: "import-book",
        label: res::str::open_book_file(),
        action: move || {
            day::task(async move {
                if let Some(file) = open_file()
                    .filter(res::str::format_epub().format(), &["epub"])
                    .await
                {
                    import(a, file).await;
                }
            });
        },
    }
    .build()
    .icon(Symbol::Open)
    .shortcut(Shortcut::new("o"))
    .enabled(move || a.database.get().is_some() && !a.busy.get())
}
static IMPORTS: std::sync::LazyLock<async_lock::Semaphore> =
    std::sync::LazyLock::new(|| async_lock::Semaphore::new(1));
async fn import(a: App, file: FileUrl) {
    let _permit = IMPORTS.acquire().await;
    a.busy.set(true);
    match file.read_limited(64 * 1024 * 1024).await {
        Ok(bytes) => {
            a.install_document(bytes, metadata::Metadata::default(), separate_reader())
                .await
        }
        Err(e) => a.error(AppError::detail(res::str::book_import_failed, e)),
    }
}
pub fn reader_window(a: App, book: epub::Book) {
    if let Some(db) = a.database.get_untracked() {
        if let Err(e) = db.opened(&book.id, now_ms() as i64) {
            a.error(e.to_string());
            return;
        }
    }
    let key = format!("reader-{}", book.id);
    let title = book.title.clone();
    day::open_window(
        Some(&key),
        day::WindowOptions {
            title,
            size: Size::new(1000., 760.),
            ..window()
        },
        day::WindowKind::Normal,
        move || {
            let reader_app = App {
                document_window: true,
                reader_open: Signal::new(None),
                reader_settings: Signal::new(false),
                book: Signal::new(None),
                ready: Signal::new(false),
                toc: Signal::new(0),
                position: Signal::new(String::new()),
                status: Signal::new(String::new()),
                busy: Signal::new(false),
                task: Signal::new(None),
                js: JsHandle::new(),
                ..a
            };
            reader_app.open(book);
            watch(
                move || a.prefs.get(),
                move |p, _| {
                    if reader_app.ready.get_untracked()
                        && !reader_app.reader_settings.get_untracked()
                    {
                        reader_app.eval(format!(
                            "stanza.configure({})",
                            serde_json::to_string(p).unwrap()
                        ));
                    }
                },
            );
            reader(reader_app)
        },
    );
}
