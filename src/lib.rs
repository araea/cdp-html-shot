/*!
[![GitHub]](https://github.com/araea/cdp-html-shot)&ensp;[![crates-io]](https://crates.io/crates/cdp-html-shot)&ensp;[![docs-rs]](crate)

[GitHub]: https://img.shields.io/badge/github-8da0cb?style=for-the-badge&labelColor=555555&logo=github
[crates-io]: https://img.shields.io/badge/crates.io-fc8d62?style=for-the-badge&labelColor=555555&logo=rust
[docs-rs]: https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs

<br>

A Rust library for capturing HTML screenshots using the Chrome DevTools Protocol (CDP).
*/

pub mod browser;
pub mod element;
#[cfg(feature = "atexit")]
pub mod exit_hook;
pub mod tab;
pub mod transport;
pub mod types;
pub mod utils;

// Re-export main types to the root
pub use browser::Browser;
pub use element::Element;
#[cfg(feature = "atexit")]
pub use exit_hook::ExitHook;
pub use tab::Tab;
pub use types::*;
