use std::io::{self, ErrorKind};
use std::sync::{Arc, Mutex};
use std::time::Duration;
// use ic_cdk::api::stable::{stable64_read, stable64_size, stable64_write};

use sqlite_vfs::{LockKind, OpenKind, OpenOptions, Vfs};
// use crate::{stable_capacity, stable_grow_bytes};
// use fil_actors_runtime::runtime::Runtime;

const SQLITE_SIZE_IN_BYTES: u64 = 8; // 8 byte

// #[derive(Default)]
pub struct PagesVfs {
    lock_state: Arc<Mutex<LockState>>,
    read: fn(offset: u64, buf: &mut [u8]),
    write: fn(offset: u64, buf: &[u8]),
}

#[derive(Debug, Default)]
struct LockState {
    read: usize,
    write: Option<bool>,
}

pub struct Connection {
    lock_state: Arc<Mutex<LockState>>,
    lock: LockKind,
    read: fn(offset: u64, buf: &mut [u8]),
    write: fn(offset: u64, buf: &[u8]),
}

impl PagesVfs {
    pub fn new_with_runtime(
        read: fn(offset: u64, buf: &mut [u8]),
        write: fn(offset: u64, buf: &[u8]),
    ) -> Self {
        PagesVfs { lock_state: Arc::new(Mutex::new(Default::default())), read, write }
    }
}

impl Vfs for PagesVfs {
    type Handle = Connection;

    fn open(&self, db: &str, opts: OpenOptions) -> Result<Self::Handle, io::Error> {
        // Always open the same database for now.
        if db != "main.db" {
            return Err(io::Error::new(
                ErrorKind::NotFound,
                format!("unexpected database name `{}`; expected `main.db`", db),
            ));
        }
        // Only main databases supported right now (no journal, wal, temporary, ...)
        if opts.kind != OpenKind::MainDb {
            return Err(io::Error::new(
                ErrorKind::PermissionDenied,
                "only main database supported right now (no journal, wal, ...)",
            ));
        }

        Ok(Connection {
            lock_state: self.lock_state.clone(),
            lock: LockKind::None,
            read: self.read,
            write: self.write,
        })
    }

    fn delete(&self, _db: &str) -> Result<(), io::Error> {
        Ok(())
    }

    fn exists(&self, db: &str) -> Result<bool, io::Error> {
        // Ok(db == "main.db" && Connection::size() > 0)
        Ok(db == "main.db")
    }

    fn temporary_name(&self) -> String {
        String::from("main.db")
    }

    fn random(&self, buffer: &mut [i8]) {
        let data =
            (0..buffer.len()).map(|_| ((123345678910 as u128) % 256) as i8).collect::<Vec<_>>();
        buffer.copy_from_slice(&data);
    }

    fn sleep(&self, duration: Duration) -> Duration {
        // let now = Instant::now;
        // conn_sleep((duration.as_millis() as u32).max(1));
        // now.elapsed()
        Duration::from_millis(1)
    }
}

impl sqlite_vfs::DatabaseHandle for Connection {
    type WalIndex = sqlite_vfs::WalDisabled;

    fn size(&self) -> Result<u64, io::Error> {
        Ok(self.size())
    }

    fn read_exact_at(&mut self, buf: &mut [u8], offset: u64) -> Result<(), io::Error> {
        if stable64_size() > 0 {
            (self.read)(offset + SQLITE_SIZE_IN_BYTES, buf);
        }
        Ok(())
    }

    fn write_all_at(&mut self, buf: &[u8], offset: u64) -> Result<(), io::Error> {
        let size = offset + buf.len() as u64;
        if size > self.size() {
            (self.write)(0, &size.to_be_bytes());
        }
        (self.write)(offset + SQLITE_SIZE_IN_BYTES, buf);
        Ok(())
    }

    fn sync(&mut self, _data_only: bool) -> Result<(), io::Error> {
        // Everything is directly written to storage, so no extra steps necessary to sync.
        Ok(())
    }

    fn set_len(&mut self, size: u64) -> Result<(), io::Error> {
        let capacity =
            if stable64_size() == 0 { 0 } else { stable_capacity() - SQLITE_SIZE_IN_BYTES };
        if size > capacity {
            stable_grow_bytes(size - capacity)
                .map_err(|err| io::Error::new(ErrorKind::OutOfMemory, err))?;
            (self.write)(0, &size.to_be_bytes());
        }
        Ok(())
    }

    fn lock(&mut self, lock: LockKind) -> Result<bool, io::Error> {
        let ok = Self::lock(self, lock);
        Ok(ok)
    }

    fn reserved(&mut self) -> Result<bool, io::Error> {
        Ok(Self::reserved(self))
    }

    fn current_lock(&self) -> Result<LockKind, io::Error> {
        Ok(self.lock)
    }

    fn wal_index(&self, _readonly: bool) -> Result<Self::WalIndex, io::Error> {
        Ok(sqlite_vfs::WalDisabled::default())
    }
}

impl Connection {
    fn size(&self) -> u64 {
        if stable64_size() == 0 {
            return 0;
        }
        let mut buf = [0u8; SQLITE_SIZE_IN_BYTES as usize];
        (self.read)(0, &mut buf);
        u64::from_be_bytes(buf)
    }

    fn lock(&mut self, to: LockKind) -> bool {
        if self.lock == to {
            return true;
        }

        let mut lock_state = self.lock_state.lock().unwrap();

        match to {
            LockKind::None => {
                if self.lock == LockKind::Shared {
                    lock_state.read -= 1;
                } else if self.lock > LockKind::Shared {
                    lock_state.write = None;
                }
                self.lock = LockKind::None;
                true
            }

            LockKind::Shared => {
                if lock_state.write == Some(true) && self.lock <= LockKind::Shared {
                    return false;
                }

                lock_state.read += 1;
                if self.lock > LockKind::Shared {
                    lock_state.write = None;
                }
                self.lock = LockKind::Shared;
                true
            }

            LockKind::Reserved => {
                if lock_state.write.is_some() || self.lock != LockKind::Shared {
                    return false;
                }

                if self.lock == LockKind::Shared {
                    lock_state.read -= 1;
                }
                lock_state.write = Some(false);
                self.lock = LockKind::Reserved;
                true
            }

            LockKind::Pending => {
                // cannot be requested directly
                false
            }

            LockKind::Exclusive => {
                if lock_state.write.is_some() && self.lock <= LockKind::Shared {
                    return false;
                }

                if self.lock == LockKind::Shared {
                    lock_state.read -= 1;
                }

                lock_state.write = Some(true);
                if lock_state.read == 0 {
                    self.lock = LockKind::Exclusive;
                    true
                } else {
                    self.lock = LockKind::Pending;
                    false
                }
            }
        }
    }

    fn reserved(&self) -> bool {
        if self.lock > LockKind::Shared {
            return true;
        }

        let lock_state = self.lock_state.lock().unwrap();
        lock_state.write.is_some()
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        if self.lock != LockKind::None {
            self.lock(LockKind::None);
        }
    }
}

fn conn_sleep(ms: u32) {
    // std::thread::sleep(Duration::from_secs(ms.into()));
}

pub fn stable64_size() -> u64 {
    512 * 1024 * 1024
}

// pub fn stable64_read(offset: u64, buf: &mut [u8]) {
//     // CANISTER_STABLE_MEMORY.stable64_read(offset, buf)
// }
//
// pub fn stable64_write(offset: u64, buf: &[u8]) {
//     // CANISTER_STABLE_MEMORY.stable64_write(offset, buf)
// }

const WASM_PAGE_SIZE_IN_BYTES: u64 = 64 * 1024; // 64KB

/// Gets capacity of the stable memory in bytes.
pub fn stable_capacity() -> u64 {
    stable64_size() << 16
}

/// Attempts to grow the memory by adding new pages.
pub fn stable_grow_bytes(size: u64) -> Result<u64, StableMemoryError> {
    let added_pages = (size as f64 / WASM_PAGE_SIZE_IN_BYTES as f64).ceil() as u64;
    stable64_grow(added_pages)
}

pub fn stable64_grow(new_pages: u64) -> Result<u64, StableMemoryError> {
    // CANISTER_STABLE_MEMORY.stable64_grow(new_pages)
    Ok(0)
}

#[derive(Debug)]
pub enum StableMemoryError {
    /// No more stable memory could be allocated.
    OutOfMemory,
    /// Attempted to read more stable memory than had been allocated.
    OutOfBounds,
}

impl std::fmt::Display for StableMemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::OutOfMemory => f.write_str("Out of memory"),
            Self::OutOfBounds => f.write_str("Read exceeds allocated memory"),
        }
    }
}

impl std::error::Error for StableMemoryError {}

pub trait StableMemory {
    /// Similar to `stable_size` but with support for 64-bit addressed memory.
    fn stable64_size(&self) -> u64;

    /// Similar to `stable_grow` but with support for 64-bit addressed memory.
    fn stable64_grow(&self, new_pages: u64) -> Result<u64, StableMemoryError>;

    /// Similar to `stable_write` but with support for 64-bit addressed memory.
    fn stable64_write(&self, offset: u64, buf: &[u8]);

    /// Similar to `stable_read` but with support for 64-bit addressed memory.
    fn stable64_read(&self, offset: u64, buf: &mut [u8]);
}

struct Memory {}

impl StableMemory for Memory {
    fn stable64_size(&self) -> u64 {
        todo!()
    }

    fn stable64_grow(&self, new_pages: u64) -> Result<u64, StableMemoryError> {
        todo!()
    }

    fn stable64_write(&self, offset: u64, buf: &[u8]) {
        todo!()
    }

    fn stable64_read(&self, offset: u64, buf: &mut [u8]) {
        todo!()
    }
}
