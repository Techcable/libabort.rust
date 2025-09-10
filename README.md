# libabort [![Latest Version]][crates.io]

<!-- cargo-rdme start -->

An implementation of the `abort` function that works without the standard library.

Provides an [`AbortGuard`] type to abort the process unless explicitly "defused".
This can prevent panics from unwinding in the middle of `unsafe` code,
which trivially makes the code [exception safe][nomicon-exception-safety].

## Available implementations
The library offers multiple possible implementations,
which can be controlled by using feature flags.

1. Using the Rust standard library [`std::process::abort`] function.
   This is enabled by using the "std" feature (disabled by default).
2. Using the C standard library [`abort`][libc-abort] function from the [`libc` crate][libc-crate].
   This requires linking against the C standard library, but not the Rust one.
   This is enabled by using the "libc" feature (disabled by default).
3. If the `panic!` implementation is known to abort instead of unwinding,
   then the `abort` function simply triggers a panic.
   This requires a recent version of Rust (1.60) in order to detect whether panics unwind or abort.
4. On Rust versions since [1.81], this unwinds past a `extern "C"` function, which is guaranteed to trigger an abort.
5. If no other implementations are available,
   then the `abort` function triggers a double-panic.
   This always triggers an abort regardless of the rust version or compiler settings.

[libc-abort]: https://en.cppreference.com/w/c/program/abort
[libc-crate]: https://crates.io/crates/libc
[nomicon-exception-safety]: https://doc.rust-lang.org/nomicon/exception-safety.html
[1.81]: https://blog.rust-lang.org/2024/09/05/Rust-1.81.0/

<!-- cargo-rdme end -->

<!--
   Need explicit links to type because `cargo rdme` has broken detection.
   TODO: File an issue for this.
-->
[`AbortGuard`]: https://docs.rs/libabort/latest/libabort/struct.AbortGuard.html
[`std::process::abort`]: https://doc.rust-lang.org/stable/std/process/fn.abort.html


[Latest Version]: https://img.shields.io/crates/v/libabort.svg
[crates.io]: https://crates.io/crates/libabort


## License
Licensed under either of Apache License, Version 2.0 or MIT license at your option.
Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this project by you,
as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
