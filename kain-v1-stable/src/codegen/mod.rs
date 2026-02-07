//! KAIN Code Generation - Multi-target output

pub mod wasm;
#[cfg(feature = "llvm")]
pub mod llvm;
pub mod spirv;
pub mod hlsl;
pub mod usf;
pub mod js;
pub mod rust;

pub use wasm::generate as generate_wasm;
#[cfg(feature = "llvm")]
pub use llvm::generate as generate_llvm;
pub use spirv::generate as generate_spirv;
pub use hlsl::generate as generate_hlsl;
pub use usf::generate as generate_usf;
pub use js::generate as generate_js;
pub use rust::generate as generate_rust;

