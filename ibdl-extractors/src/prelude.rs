pub use crate::extractor::caps::AsyncFetch;
pub use crate::extractor::caps::Auth;
pub use crate::extractor::caps::ExtractorFeatures;

#[cfg(not(target_arch = "wasm32"))]
pub use crate::extractor::caps::ExtractorThreadHandle;

pub use crate::extractor::caps::PoolExtract;
pub use crate::extractor::caps::PostFetchAsync;
pub use crate::extractor::caps::PostFetchMethod;
pub use crate::extractor::caps::SinglePostFetch;
