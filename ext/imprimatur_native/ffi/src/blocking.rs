//! Work run without the GVL that cannot stop midway (compressing a file):
//! Ruby's other threads keep running, and an interrupt waits for the end.

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

use magnus::{Error, Ruby};

/// Runs interrupted before they start are retried this many times.
pub const MAX_ATTEMPTS: usize = 64;

/// The job, and what it returned (or that it panicked).
struct Slot<F, T> {
    job: Option<F>,
    result: Option<std::thread::Result<T>>,
}

/// Runs `job` on this thread with the GVL released.
pub fn without_gvl<F: FnOnce() -> T, T>(ruby: &Ruby, job: F) -> Result<T, Error> {
    let mut slot = Slot { job: Some(job), result: None };
    let pointer: *mut Slot<F, T> = &raw mut slot;
    for _ in 0..MAX_ATTEMPTS {
        // SAFETY: `slot` outlives the call, which returns once `run` has
        // finished or never started; `run` touches no Ruby object. Without
        // an unblocking function the job runs to its end; the `2` variant
        // raises no pending interrupt across these Rust frames.
        #[allow(unsafe_code)]
        unsafe {
            rb_sys::rb_thread_call_without_gvl2(
                Some(run::<F, T>),
                pointer.cast(),
                None,
                std::ptr::null_mut(),
            );
        }
        match slot.result.take() {
            Some(Ok(value)) => return Ok(value),
            Some(Err(_)) => {
                return Err(Error::new(
                    ruby.exception_runtime_error(),
                    "imprimatur failed on this archive",
                ));
            }
            // An interrupt was pending, so the job never started: let Ruby
            // handle it (it may raise), then start.
            None => ruby.thread_check_ints()?,
        }
    }
    Err(Error::new(ruby.exception_runtime_error(), "interrupted before it could start"))
}

/// The job itself, without the GVL; a panic becomes an error, not an
/// abort at this C boundary.
#[allow(unsafe_code)]
unsafe extern "C" fn run<F: FnOnce() -> T, T>(data: *mut c_void) -> *mut c_void {
    // SAFETY: `data` is the `Slot` without_gvl passed, alive and not
    // otherwise borrowed until this returns.
    let slot = unsafe { &mut *data.cast::<Slot<F, T>>() };
    if let Some(job) = slot.job.take() {
        slot.result = Some(catch_unwind(AssertUnwindSafe(job)));
    }
    std::ptr::null_mut()
}
