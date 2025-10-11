pub mod control;
pub mod model_specific;
pub mod msi;
pub mod segment;
pub mod status;

pub trait Register<T> {
    fn read() -> Self;
    fn write(&self);
    fn raw(&self) -> T;
    fn set_raw(&mut self, value: T);
}
