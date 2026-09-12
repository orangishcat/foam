use std::{
    collections::HashMap,
    io,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
};

type ThreadId = u64;

#[derive(Default)]
struct ThreadRegistry {
    shutting_down: bool,
    handles: HashMap<ThreadId, JoinHandle<()>>,
}

static THREADS: LazyLock<Mutex<ThreadRegistry>> =
    LazyLock::new(|| Mutex::new(ThreadRegistry::default()));

static NEXT_THREAD_ID: AtomicU64 = AtomicU64::new(1);

struct RemoveOnDrop(ThreadId);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let mut registry = THREADS.lock().unwrap_or_else(|e| e.into_inner());
        registry.handles.remove(&self.0);
    }
}

pub fn spawn_thread(
    name: impl Into<String>,
    f: impl FnOnce() + Send + 'static,
) -> io::Result<ThreadId> {
    let id = NEXT_THREAD_ID.fetch_add(1, Ordering::Relaxed);
    let (start_tx, start_rx) = mpsc::channel();

    // unpoision the mutex
    let mut registry = THREADS.lock().unwrap_or_else(|e| e.into_inner());

    if registry.shutting_down {
        return Err(io::Error::other(
            "program is shutting down",
        ));
    }

    let handle = thread::Builder::new().name(name.into()).spawn(move || {
        // if the thread completes before it enters the registry, don't add it to the registry
        if start_rx.recv().is_err() {
            return;
        }

        let _remove_on_drop = RemoveOnDrop(id);
        f();
    })?;

    registry.handles.insert(id, handle);
    start_tx.send(()).expect("new worker unexpectedly exited");

    Ok(id)
}

pub fn join_all_threads() {
    let handles = {
        let mut registry = THREADS.lock().unwrap_or_else(|e| e.into_inner());
        registry.shutting_down = true;
        std::mem::take(&mut registry.handles)
        // regsitry is discarded, freeing up the registry lock for finishing workers
    };

    for (id, handle) in handles {
        if let Err(payload) = handle.join() {
            log::error!("thread {id} panicked: {payload:?}");
        }
    }
}
