// SPDX-License-Identifier: MPL-2.0

//! Atomic 64-bit integers, including an IRQ-safe fallback for RV32.

pub use core::sync::atomic::Ordering;
#[cfg(target_has_atomic = "64")]
pub use core::sync::atomic::{AtomicI64, AtomicU64};

#[cfg(not(target_has_atomic = "64"))]
pub use fallback::{AtomicI64, AtomicU64};

#[cfg(not(target_has_atomic = "64"))]
mod fallback {
    use core::{
        cell::UnsafeCell,
        fmt,
        sync::atomic::{AtomicBool, Ordering, fence},
    };

    // Keep the lock outside each integer so its size and alignment remain eight
    // bytes. Disable local interrupts before taking it: an interrupt handler can
    // use these atomics too. No allocation, CPU-local state or scheduler is needed.
    static LOCK: AtomicBool = AtomicBool::new(false);

    struct Guard {
        _irq: crate::irq::DisabledLocalIrqGuard,
    }

    impl Guard {
        fn new() -> Self {
            let irq = crate::irq::disable_local();
            while LOCK
                .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_err()
            {
                core::hint::spin_loop();
            }
            // All fallback operations provide sequential consistency, including
            // ordering with native atomics and fences used elsewhere in OSTD.
            fence(Ordering::SeqCst);
            Self { _irq: irq }
        }
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            fence(Ordering::SeqCst);
            LOCK.store(false, Ordering::Release);
            // The IRQ guard is dropped after releasing the lock.
        }
    }

    macro_rules! atomic_integer {
        ($name:ident, $integer:ty) => {
            /// A 64-bit atomic integer serialized with an IRQ-safe global lock.
            ///
            /// Operations accept native atomic orderings and provide sequential
            /// consistency. The fallback is not lock-free and must not be used
            /// from non-maskable interrupt handlers.
            #[repr(C, align(8))]
            pub struct $name(UnsafeCell<$integer>);

            // SAFETY: Every shared access is serialized by LOCK with local IRQs
            // disabled. Mutable access requires exclusive ownership of the value.
            unsafe impl Sync for $name {}

            impl $name {
                /// Creates an atomic integer.
                pub const fn new(value: $integer) -> Self {
                    Self(UnsafeCell::new(value))
                }

                /// Returns the value under an exclusive borrow.
                pub fn get_mut(&mut self) -> &mut $integer {
                    self.0.get_mut()
                }

                /// Consumes the atomic and returns its value.
                pub fn into_inner(self) -> $integer {
                    self.0.into_inner()
                }

                /// Returns a raw pointer; accesses must respect atomic synchronization.
                pub const fn as_ptr(&self) -> *mut $integer {
                    self.0.get()
                }

                fn with_value<R>(&self, operation: impl FnOnce(&mut $integer) -> R) -> R {
                    let _guard = Guard::new();
                    // SAFETY: The global lock excludes all other accesses and
                    // interrupts cannot re-enter this critical section locally.
                    operation(unsafe { &mut *self.0.get() })
                }

                /// Loads the value.
                pub fn load(&self, order: Ordering) -> $integer {
                    assert!(matches!(
                        order,
                        Ordering::Relaxed | Ordering::Acquire | Ordering::SeqCst
                    ));
                    self.with_value(|value| *value)
                }

                /// Stores a new value.
                pub fn store(&self, new: $integer, order: Ordering) {
                    assert!(matches!(
                        order,
                        Ordering::Relaxed | Ordering::Release | Ordering::SeqCst
                    ));
                    self.with_value(|value| *value = new);
                }

                /// Replaces the value, returning its previous value.
                pub fn swap(&self, new: $integer, _order: Ordering) -> $integer {
                    self.with_value(|value| core::mem::replace(value, new))
                }

                /// Replaces the value only if it matches `current`.
                pub fn compare_exchange(
                    &self,
                    current: $integer,
                    new: $integer,
                    _success: Ordering,
                    failure: Ordering,
                ) -> Result<$integer, $integer> {
                    assert!(matches!(
                        failure,
                        Ordering::Relaxed | Ordering::Acquire | Ordering::SeqCst
                    ));
                    self.with_value(|value| {
                        if *value == current {
                            Ok(core::mem::replace(value, new))
                        } else {
                            Err(*value)
                        }
                    })
                }

                /// Compares and exchanges; this fallback does not fail spuriously.
                pub fn compare_exchange_weak(
                    &self,
                    current: $integer,
                    new: $integer,
                    success: Ordering,
                    failure: Ordering,
                ) -> Result<$integer, $integer> {
                    self.compare_exchange(current, new, success, failure)
                }

                /// Retries a conditional update, returning the previous value.
                pub fn fetch_update(
                    &self,
                    set_order: Ordering,
                    fetch_order: Ordering,
                    mut update: impl FnMut($integer) -> Option<$integer>,
                ) -> Result<$integer, $integer> {
                    let mut old = self.load(fetch_order);
                    loop {
                        // User code runs outside the lock and may use other atomics.
                        let Some(new) = update(old) else {
                            return Err(old);
                        };
                        match self.compare_exchange(old, new, set_order, fetch_order) {
                            Ok(value) => return Ok(value),
                            Err(value) => old = value,
                        }
                    }
                }

                /// Retries a conditional update, returning the previous value.
                pub fn try_update(
                    &self,
                    set_order: Ordering,
                    fetch_order: Ordering,
                    update: impl FnMut($integer) -> Option<$integer>,
                ) -> Result<$integer, $integer> {
                    self.fetch_update(set_order, fetch_order, update)
                }

                /// Retries an update, returning the previous value.
                pub fn update(
                    &self,
                    set_order: Ordering,
                    fetch_order: Ordering,
                    mut update: impl FnMut($integer) -> $integer,
                ) -> $integer {
                    self.fetch_update(set_order, fetch_order, |old| Some(update(old)))
                        .unwrap()
                }

                /// Adds with wrapping arithmetic, returning the previous value.
                pub fn fetch_add(&self, value: $integer, _order: Ordering) -> $integer {
                    self.with_value(|old| {
                        let result = *old;
                        *old = old.wrapping_add(value);
                        result
                    })
                }
                /// Subtracts with wrapping arithmetic, returning the previous value.
                pub fn fetch_sub(&self, value: $integer, _order: Ordering) -> $integer {
                    self.with_value(|old| {
                        let result = *old;
                        *old = old.wrapping_sub(value);
                        result
                    })
                }
                /// Applies bitwise OR, returning the previous value.
                pub fn fetch_or(&self, value: $integer, _order: Ordering) -> $integer {
                    self.with_value(|old| {
                        let result = *old;
                        *old |= value;
                        result
                    })
                }
                /// Applies bitwise AND, returning the previous value.
                pub fn fetch_and(&self, value: $integer, _order: Ordering) -> $integer {
                    self.with_value(|old| {
                        let result = *old;
                        *old &= value;
                        result
                    })
                }
                /// Applies bitwise XOR, returning the previous value.
                pub fn fetch_xor(&self, value: $integer, _order: Ordering) -> $integer {
                    self.with_value(|old| {
                        let result = *old;
                        *old ^= value;
                        result
                    })
                }
                /// Applies bitwise NAND, returning the previous value.
                pub fn fetch_nand(&self, value: $integer, _order: Ordering) -> $integer {
                    self.with_value(|old| {
                        let result = *old;
                        *old = !(*old & value);
                        result
                    })
                }
                /// Stores the smaller value, returning the previous value.
                pub fn fetch_min(&self, value: $integer, _order: Ordering) -> $integer {
                    self.with_value(|old| {
                        let result = *old;
                        *old = (*old).min(value);
                        result
                    })
                }
                /// Stores the larger value, returning the previous value.
                pub fn fetch_max(&self, value: $integer, _order: Ordering) -> $integer {
                    self.with_value(|old| {
                        let result = *old;
                        *old = (*old).max(value);
                        result
                    })
                }
            }

            impl Default for $name {
                fn default() -> Self {
                    Self::new(0)
                }
            }
            impl From<$integer> for $name {
                fn from(value: $integer) -> Self {
                    Self::new(value)
                }
            }
            impl fmt::Debug for $name {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    self.load(Ordering::Relaxed).fmt(f)
                }
            }
        };
    }

    atomic_integer!(AtomicU64, u64);
    atomic_integer!(AtomicI64, i64);
}
