// only enables the `doc_cfg` feature when the `docsrs` configuration attribute is defined
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(missing_docs)]
#![deny(clippy::all)]
#![deny(clippy::pedantic)]
#![forbid(unsafe_code)]

//! Facilitates the synchronization of threads.
//!
//! The struct [`CondSync`] is a thin wrapper around
//! [`Arc`]`<(`[`Mutex`]`<T>, `[`Condvar`]`)>` and hides boiler plate code
//! that is needed when using `std::sync::Condvar` directly.
use std::{
    sync::{Arc, Condvar, Mutex, PoisonError},
    time::{Duration, Instant},
};

/// A thin wrapper around [`Arc`]`<(`[`Mutex`]`<T>, `[`Condvar`]`)>`.
///
/// It enhances readability when synchronizing threads
/// (compare with the examples given for [`Condvar`]).
///
/// ## Example: Inform main thread when all child threads have initialized:
///
/// ```rust
/// use cond_sync::{CondSync, Other};
/// use std::{thread, time::Duration};
///
/// // we use here a plain usize as condition state:
/// let cond_sync = CondSync::new(0_usize);
///
/// for i in 0..5 {
///     let cond_sync_t = cond_sync.clone();
///     thread::spawn(move || {
///         println!("Thread {i}: initializing ...");
///         // modify the state:
///         cond_sync_t.modify_and_notify(|v| *v += 1, Other::One).unwrap();
///
///         thread::sleep(Duration::from_millis(1)); // just to produce a yield
///         println!("Thread {i}: work on phase 1");
///     });
/// }
/// // [main thread] wait here until the condition is fulfilled:
/// cond_sync.wait_until(|v| *v == 5).unwrap();
///
/// println!("Main: All threads initialized");
/// thread::sleep(Duration::from_millis(100)); // just to let the threads finish (better use join)
/// ```
///
/// prints something like
///
/// ```text
/// Thread 0: initializing ...
/// Thread 2: initializing ...
/// Thread 1: initializing ...
/// Thread 3: initializing ...
/// Thread 4: initializing ...
/// Main: All threads initialized
/// Thread 2: work on phase 1
/// Thread 0: work on phase 1
/// Thread 1: work on phase 1
/// Thread 4: work on phase 1
/// Thread 3: work on phase 1
/// ```
///
pub struct CondSync<T>(Arc<I<T>>);

struct I<T> {
    mtx: Mutex<T>,
    cvar: Condvar,
}

impl<T> CondSync<T> {
    /// Construct a new instance, based on the variable you logically need to manage the synchronization.
    pub fn new(value: T) -> Self {
        Self(Arc::new(I {
            mtx: Mutex::new(value),
            cvar: Condvar::new(),
        }))
    }

    /// Blocks the current thread until the given condition,
    /// when called with the current value of the wrapped variable, returns `true`.
    ///
    /// ## Errors
    ///
    /// This function will return an error if the internally used mutex being waited on is
    /// poisoned when this thread tries to re-acquire the lock.
    /// For more information, see information about poisoning on the Mutex type.
    pub fn wait_until<F>(&self, condition: F) -> Result<Reason, PoisonedError>
    where
        F: Fn(&T) -> bool,
    {
        let mut mtx_guard = self.0.mtx.lock()?;
        while !condition(&*mtx_guard) {
            mtx_guard = self.0.cvar.wait(mtx_guard)?;
        }
        Ok(Reason::Condition)
    }

    /// Blocks the current thread until the given test method,
    /// when called with the current value of the wrapped variable, returns `true`, but no longer
    /// than the given duration.
    ///
    /// ## Returns
    ///
    /// Returns `true` if the timeout was reached, and `false` if the condition was fulfilled.
    ///
    /// ## Errors
    ///
    /// This function will return an error if the internally used mutex being waited on is
    /// poisoned when this thread re-acquires the lock.
    /// For more information, see information about poisoning on the Mutex type.
    pub fn wait_until_or_timeout<F>(
        &self,
        condition: F,
        duration: Duration,
    ) -> Result<Reason, PoisonedError>
    where
        F: Fn(&T) -> bool,
    {
        let mut mtx_guard = self.0.mtx.lock()?;
        let end = Instant::now() + duration;
        while !condition(&*mtx_guard) {
            let now = Instant::now();
            match self.0.cvar.wait_timeout(mtx_guard, end - now) {
                Ok((mtxg, wtr)) => {
                    if wtr.timed_out() {
                        return Ok(Reason::Timeout);
                    }
                    mtx_guard = mtxg;
                }
                Err(_) => return Err(PoisonedError),
            }
        }
        Ok(Reason::Condition)
    }

    /// Blocks the current thread until a notification is received, but no longer
    /// than the given duration.
    ///
    /// ## Returns
    ///
    /// Returns `true` if the timeout was reached, and `false` otherwise.
    ///
    /// ## Errors
    ///
    /// This function will return an error if the internally used mutex being waited on is
    /// poisoned when this thread re-acquires the lock.
    /// For more information, see information about poisoning on the Mutex type.
    pub fn wait_timeout(&self, duration: Duration) -> Result<Reason, PoisonedError> {
        let mtx_guard = self.0.mtx.lock()?;
        let end = Instant::now() + duration;

        Ok(self
            .0
            .cvar
            .wait_timeout(mtx_guard, end - Instant::now())
            .map(|(_, wtr)| {
                if wtr.timed_out() {
                    Reason::Timeout
                } else {
                    Reason::Notification
                }
            })?)
    }

    /// Applies a change to the wrapped variable (by calling the given function `modify`) and
    /// notifies one or all of the other affected threads, depending on the value of `other`.
    ///
    /// ## Errors
    ///
    /// This function will return an error if the internally used mutex being waited on is
    /// poisoned when this thread re-acquires the lock.
    /// For more information, see information about poisoning on the Mutex type.
    pub fn modify_and_notify<F>(&self, modify: F, other: Other) -> Result<(), PoisonedError>
    where
        F: Fn(&mut T),
    {
        let mut mtx_guard = self.0.mtx.lock()?;
        modify(&mut *mtx_guard);
        match other {
            Other::One => self.0.cvar.notify_one(),
            Other::All => self.0.cvar.notify_all(),
        }
        Ok(())
    }
}

impl<T> Clone for CondSync<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T> CondSync<T>
where
    T: Clone,
{
    /// Produces a detached clone of the contained variable.
    #[must_use]
    pub fn clone_inner(&self) -> T {
        self.0
            .mtx
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// Helper enum to decide if one or all of the other threads should be notified.
#[derive(Copy, Clone)]
pub enum Other {
    /// One of the other threads should be notified.
    One,
    /// All other threads should be notified.
    All,
}

/// Describes why the method returned (if it returned successfully).
#[derive(Copy, Clone)]
pub enum Reason {
    /// The timeout was reached.
    Timeout,
    /// A notification was received and the condition is fulfilled.
    Condition,
    /// A notification was received.
    Notification,
}
impl Reason {
    /// Convenience method to check the concrete reason.
    #[must_use]
    pub fn is_timeout(&self) -> bool {
        matches!(&self, Self::Timeout)
    }
    /// Convenience method to check the concrete reason.
    #[must_use]
    pub fn is_condition(&self) -> bool {
        matches!(&self, Self::Condition)
    }
    /// Convenience method to check the concrete reason.
    #[must_use]
    pub fn is_notification(&self) -> bool {
        matches!(&self, Self::Notification)
    }
}

/// The inner mutex got poisoned.
///
/// This most likely happens if one of the provided closures panics.
#[derive(Debug)]
pub struct PoisonedError;
impl<T> From<PoisonError<T>> for PoisonedError {
    fn from(_e: PoisonError<T>) -> PoisonedError {
        PoisonedError
    }
}
