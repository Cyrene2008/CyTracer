pub mod detector;
pub mod proxy;

pub use detector::detect;
pub use proxy::{ensure_proxy, proxy_path};
