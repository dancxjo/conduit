//! Browser realization of recorder reads. Root retains actual storage, kernel,
//! current Body/Play custody and admitted memory/work; this creates none of them.
mod reads;
pub use reads::*;
