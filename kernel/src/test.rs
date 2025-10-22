#[allow(dead_code)]
pub trait Testable {
    fn run(&self);
}

impl<T: Fn()> Testable for T {
    fn run(&self) {
        use crate::{print, println};

        print!("{}...", core::any::type_name::<T>());
        self();
        println!("[ok]");
    }
}

#[cfg(test)]
pub fn test_runner(tests: &[&dyn Testable]) {
    use crate::{debug::qemu, println};

    println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }

    qemu::exit(qemu::EXIT_SUCCESS);
}
