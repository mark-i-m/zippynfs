
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use memmap::{Mmap, Protection};

#[derive(Debug)]
pub struct AtomicPersistentUsize<'a> {
    counter: &'a AtomicUsize,
    mmap: Mutex<Mmap>,
}

impl<'a> AtomicPersistentUsize<'a> {
    /// Creates an atomic persistent usize persisted to the given file and returns it.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<AtomicPersistentUsize<'a>, String> {
        // Mmap the file to memory
        let mut file_mmap = Mmap::open_path(path, Protection::ReadWrite).map_err(|e| {
            format!("{}", e)
        })?;

        // FIXME: This is kind of sketchy... nothing guarantees that AtomicUsize can be
        // persisted like this... it is an opaque data structure...
        let atomic = unsafe {
            let ptr = file_mmap.mut_ptr() as *mut AtomicUsize;
            &mut *ptr
        };

        Ok(AtomicPersistentUsize {
            counter: atomic,
            mmap: Mutex::new(file_mmap),
        })
    }

    /// Atomically fetch the value of the counter and add 1. Then flush to disk.
    pub fn fetch_inc(&mut self) -> usize {
        // NOTE: We must lock before fetch_add because the counter inc and the flush
        // need to happen atomically.
        // This is important for persistance on disk especially for
        // the crash recovery
        let locked = self.mmap.lock().unwrap();

        // NOTE: This does need to be atomic since we need cache coherency!
        let val = self.counter.fetch_add(1, Ordering::SeqCst);

        // NOTE: If this fails, blow up the world??
        locked.flush().unwrap();

        val
    }
}
