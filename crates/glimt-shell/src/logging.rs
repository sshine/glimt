//! One logging setup for both candidates.
//!
//! The Scheme runtime JIT compiles through cranelift, which reports every function it
//! defines at info level, in cranelift's own IR. That is a page of output per
//! expression and it drowns everything this project has to say, so libraries are quiet
//! by default and this project is not. `RUST_LOG` overrides the lot.

use tracing_subscriber::EnvFilter;

const DEFAULT: &str = "warn,glimt_shell=info,napkin_math=info,glimt=info,glimt_raw=info";

pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT));

    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_target(false)
        .with_env_filter(filter)
        .init();
}
