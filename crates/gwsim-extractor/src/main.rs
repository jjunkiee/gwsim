//! The wiki extractor: crawler, seeder and change report.
//!
//! A developer tool. It is **not shipped** with gwsim, because the data
//! it produces is committed to the repository instead (D8).
//!
//! See `docs/DESIGN.md` section 9 for the crawl rules EXT-1 to EXT-7.

use clap::Parser;

/// The gwsim wiki extractor (developer tool; not shipped).
#[derive(Debug, Parser)]
#[command(
    name = "gwsim-extract",
    version,
    about = "gwsim wiki extractor (developer tool; not shipped)"
)]
struct Cli;

fn main() {
    let _ = Cli::parse();
    println!("gwsim-extract {}", env!("CARGO_PKG_VERSION"));
    println!("developer tool; not shipped");
}
