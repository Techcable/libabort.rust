//! An implementation of the `abort` function that works without the standard library.
//!
//! Provides an [`AbortGuard`] type to abort the process unless explicitly "defused".
//! This can prevent panics from unwinding in the middle of `unsafe` code,
//! which trivially makes the code [exception safe][nomicon-exception-safety].
//!
//! # Available implementations
//! The library offers multiple possible implementations,
//! which can be controlled by using feature flags.
//!
//! 1. Using the Rust standard library [`std::process::abort`] function.
//!    This is enabled by using the "std" feature (disabled by default).
//! 2. Using the C standard library [`abort`][libc-abort] function from the [`libc` crate][libc-crate].
//!    This requires linking against the C standard library, but not the Rust one.
//!    This is enabled by using the "libc" feature (disabled by default).
//! 3. If the `panic!` implementation is known to abort instead of unwinding,
//!    then the `abort` function simply triggers a panic.
//!    This requires a recent version of Rust (1.60) in order to detect whether panics unwind or abort.
//! 4. On Rust versions since [1.81], this unwinds past a `extern "C"` function, which is guaranteed to trigger an abort.
//! 5. If no other implementations are available,
//!    then the `abort` function triggers a double-panic.
//!    This always triggers an abort regardless of the rust version or compiler settings.
//!
//! [libc-abort]: https://en.cppreference.com/w/c/program/abort
//! [libc-crate]: https://crates.io/crates/libc
//! [nomicon-exception-safety]: https://doc.rust-lang.org/nomicon/exception-safety.html
//! [1.81]: https://blog.rust-lang.org/2024/09/05/Rust-1.81.0/
#![cfg_attr(not(any(doc, feature = "std")), no_std)]
#![cfg_attr(has_doc_cfg, feature(doc_cfg))] // doc_cfg only supported on nightly
#![cfg_attr(trap_impl = "core-intrinsics", allow(internal_features))] // very stable in practice...
#![cfg_attr(trap_impl = "core-intrinsics", feature(core_intrinsics))]
#![cfg_attr(trap_impl = "wasm64-intrinsic", feature(simd_wasm64))] // currently unstable
#![deny(
    dead_code, // Don't allow missing implementations
    clippy::undocumented_unsafe_blocks,
)]

/// Abort the process, as if calling [`std::process::abort`]
/// or the C standard library [`abort`](https://en.cppreference.com/w/c/program/abort) function.
///
/// This immediately terminates the process,
/// without calling any destructors or exit codes.
///
///
/// ## Implementations
/// The preferred implementations delegate to a platform-specific abort function.
/// This is enabled whenever `feature = "std"` or `feature = "libc"` is enabled.
/// Using a preferred implementation is equivalent to calling [`immediate_abort`].
///
/// When a platform-specific abort function is not available,
/// this will fall back to using a `panic!` as described in the below section.
///
/// ## Safety
/// This function is **guaranteed** to terminate the process.
/// Unlike the `panic!` function,
/// this function will never unwind into caller code.
///
/// After aborting, this process will never execute any further user code.
/// It is possible some `panic!` code will run inside this function.
/// See the section below for more details.
///
/// ### Invoking `panic!` as fallback
/// No user code will run after invoking this function.
/// However, one of the fallback implementations uses [`core::panic!`] internally.
/// This will trigger the panic hook and run code from the standard library.
/// Outside a call to an `abort` function,
/// this is guaranteed to be the only other code invoked.
/// It will always be passed a `&'static str` argument,
/// which reduces or eliminates use of `core::fmt` machinery.
///
/// For safe code this shouldn't be much of an issue
/// unless you are bothered by the panic hook printing to standard error.
/// To avoid printing to standard output,
/// the easiest workaround is to enable an alternate implementation (see below).
///
/// For unsafe code, there may be further problems.
/// If `unsafe` invariants have been violated,
/// it may be unsafe to execute any code whatever
/// and the abort must be immediate.
///
/// If this usage is unacceptable, invoke the [`immediate_abort`] function instead.
/// This function will never use the fallback implementation.
/// If the primary implementation is missing,
/// it will simply be missing from the library (removed with `cfg`).
/// Alternatively, using the stdlib "panic_immediate_abort" feature should have a similar effect
/// and using the fallback implementation will be fine.
/// As a third choice, `feature = "always-immediate-abort"` will trigger a global compilation error
/// rather than use the fallback implementation.
#[cold]
#[inline(always)] // immediately delegates
pub fn abort() -> ! {
    #[cfg(not(abort_impl = "fallback"))]
    {
        immediate_abort()
    }
    // fallback
    #[cfg(abort_impl = "fallback")]
    {
        fallback_abort()
    }
}

/// Immediately call the platform-specific [`abort`] implementation,
/// without invoking any other code.
///
/// Unlike [`abort`], this will never use a fallback implementation that calls `panic!`.
/// Instead, this function will simply not exist.
///
/// In most cases (especially safe code),
/// using the regular [`abort`] function is fine.
#[cfg(not(abort_impl = "fallback"))]
// NOTE: Keep doc(cfg(...)) in sync with the underlying reasons for the abort_impl
#[cfg_attr(
    has_doc_cfg,
    doc(cfg(any(feature = "std", feature = "libc", feature = "abort-via-trap")))
)]
#[inline(always)] // immediately delegates
pub fn immediate_abort() -> ! {
    // implicitly requires std
    #[cfg(abort_impl = "std")]
    {
        std::process::abort();
    }
    // use standard C library abort function
    // SAFETY: libc::abort() is safe to invoke
    #[cfg(abort_impl = "libc")]
    unsafe {
        libc::abort();
    }
    // abort by doing a trap instruction
    #[cfg(abort_impl = "trap")]
    {
        invoke_trap()
    }
}

/// The fallback implementation.
///
/// ## Rationale for never inlining
/// The most important reason this function should never be inlined
/// is because calling `panic!` might trigger unwinding.
/// We want to guarantee this never happens.
///
/// The secondary reason is that inlining this code would bloat the caller
/// and aborts should always be on the cold-path.
/// The double-panic implementation is two direct calls instead of one.
/// Even the `panic="abort"` case is not inlined,
/// because calling a single-argument function
/// requires an additional load & register move
/// than calling a zero-argument function.
#[cfg(abort_impl = "fallback")]
#[inline(never)]
#[cold]
fn fallback_abort() -> ! {
    #[cfg(feature = "always-immediate-abort")]
    {
        compile_error!("Missing `immediate_abort()` implementation but fallback disabled.")
    }
    #[inline(always)]
    fn do_panic() -> ! {
        panic!("fatal error - aborting");
    }
    /// Unwinds past an `extern "C"` boundary.
    ///
    /// ## Safety
    /// Must be a rust version where unwinding past
    /// ABI boundaries is guarenteed to abort.
    ///
    /// If this guarentee doesn't hold,
    /// this is undefined behavior.
    #[inline(never)]
    unsafe extern "C" fn cabi_unwind() -> ! {
        do_panic();
    }
    /*
     * Check if a panics cause unwinding or immediate aborts.
     * If it aborts, we only need to panic once.
     * If it unwinds, we need to do a double-panic.
     *
     * NOTE: cfg!(panic = "abort") was stabilized in rust 1.60.0.
     * While unknown cfg!(...) attributes would normally evaluate to false,
     * for a couple of versions even mentioning this attribute required
     * a nightly compiler.
     * To avoid errors on old stable compilers,
     * we gate on the compiler version with #[rustversion::since(...))]
     */
    const PANIC_DOES_ABORT: bool = {
        #[cfg(has_cfg_panic)]
        {
            cfg!(panic = "abort")
        }
        #[cfg(not(has_cfg_panic))]
        {
            false
        }
    };
    if PANIC_DOES_ABORT {
        do_panic()
    } else if cfg!(is_cabi_unwind_guarenteed_abort) {
        // SAFETY: On rust versions >= 1.81,
        // unwinding past C ABI boundaries is guarenteed to abort
        unsafe { cabi_unwind() }
    } else {
        // double panics are guarenteed to abort
        struct DoublePanicGuard;
        impl Drop for DoublePanicGuard {
            #[inline]
            fn drop(&mut self) {
                do_panic(); // this will abort the process
            }
        }
        let _guard = DoublePanicGuard;
        do_panic()
    }
}

/// Abnormally terminate the program by generating a trap instruction.
///
/// This is semantically equivalent to the [LLVM `llvm.trap` intrinsic][llvm-trap].
///
/// One advantage over calling the [`abort`] function is that
/// the caller code-size is often smaller.
///
/// [llvm-trap]: https://releases.llvm.org/18.1.0/docs/LangRef.html#llvm-trap-intrinsic
#[inline(always)]
#[cold]
pub fn trap() -> ! {
    #[cfg(not(trap_impl = "fallback"))]
    {
        invoke_trap()
    }
    #[cfg(trap_impl = "fallback")]
    {
        abort()
    }
}

/// Actually invoke the underlying trap instruction.
///
/// When using the "fallback" trap implementation,
/// this function is missing.
#[inline(always)]
#[cfg(not(trap_impl = "fallback"))]
#[cold]
fn invoke_trap() -> ! {
    #[cfg(trap_impl = "wasm32-intrinsic")]
    {
        // The `wasm32` module is stabilized under feature "simd_wasm32", accepted in 1.33
        core::arch::wasm32::unreachable()
    }

    #[cfg(trap_impl = "wasm64-intrinsic")]
    {
        // The `wasm64` module is currently unstable
        //
        // TODO: Test this architecture (issue #3)
        core::arch::wasm64::unreachable()
    }
    #[cfg(trap_impl = "core-intrinsics")]
    {
        core::intrinsics::abort()
    }
    #[cfg(trap_impl = "assembly")]
    unsafe {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            core::arch::asm!("ud2", options(noreturn, nomem, nostack));
        }
        #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
        {
            // On aarch64:
            // GCC __builtin_trap() does `brk #1000`
            // LLVM __builtin_trap() does `brk #0x1`
            // On ARM32:
            // LLVM does `.inst 0xe7ffdefe`
            //
            // However in all cases, `udf` works just as well.
            // It is a shorthand for an undefined instruction
            // Also `brk` is sometimes used to trigger debuggers
            core::arch::asm!("udf #0xDEAD", options(noreturn, nomem, nostack));
        }
    }
}

/// A RAII guard that [aborts](`abort`) the process unless it is explicitly [defused](`AbortGuard::defuse`).
///
/// This is very useful for guaranteeing a section of code will never panic,
/// trivially ensuring the [exception
/// safety](https://doc.rust-lang.org/nomicon/exception-safety.html) of unsafe code.
#[derive(Debug, Clone)]
pub struct AbortGuard {
    _priv: (),
}
impl AbortGuard {
    /// Create a new abort guard,
    /// which will trigger an abort unless [`defuse`](Self::defuse) is called.
    #[inline]
    pub fn new() -> Self {
        AbortGuard { _priv: () }
    }

    /// Defuse the guard, preventing the drop function from calling [`abort`].
    ///
    /// This is typically used after succesfull execution of some code.
    #[inline]
    pub fn defuse(self) {
        core::mem::forget(self)
    }
}
impl Default for AbortGuard {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
impl Drop for AbortGuard {
    #[cold]
    #[inline]
    fn drop(&mut self) {
        crate::abort();
    }
}
