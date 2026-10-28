//! The optional Apple symbols build input, `APPLE_SYMBOLS`: icon name and a square, tintable SVG.
//! Empty unless a folder of SF Symbols exports was there when the crate was built (see
//! `build.rs`); [`crate::icons`] then draws every icon in it with the symbol, and the rest with
//! Lucide.

include!(concat!(env!("OUT_DIR"), "/apple_symbols.rs"));
