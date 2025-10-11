pub use control::*;
pub use model_specific::*;
pub use msi::*;
pub use segment::*;
pub use status::*;

mod control;
mod model_specific;
mod msi;
pub mod segment;
mod status;

pub trait Register<T> {
    fn read() -> Self;
    fn write(&self);
    fn raw(&self) -> T;
    fn set_raw(&mut self, value: T);
}
