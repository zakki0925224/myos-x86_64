use core::marker::PhantomData;
use core::sync::atomic::{AtomicUsize, Ordering};

pub trait AtomicIdMarker: 'static {}

#[derive(Debug, Clone, Copy)]
pub struct AtomicId<T: AtomicIdMarker>(usize, PhantomData<T>);

impl<T: AtomicIdMarker> PartialEq for AtomicId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }

    fn ne(&self, other: &Self) -> bool {
        self.get() != other.get()
    }
}

impl<T: AtomicIdMarker> Eq for AtomicId<T> {}

impl<T: AtomicIdMarker> PartialOrd for AtomicId<T> {
    fn lt(&self, other: &Self) -> bool {
        self.get() < other.get()
    }

    fn le(&self, other: &Self) -> bool {
        self.get() <= other.get()
    }

    fn gt(&self, other: &Self) -> bool {
        self.get() > other.get()
    }

    fn ge(&self, other: &Self) -> bool {
        self.get() >= other.get()
    }

    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.get().partial_cmp(&other.get())
    }
}

impl<T: AtomicIdMarker> Ord for AtomicId<T> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.get().cmp(&other.get())
    }
}

impl<T: AtomicIdMarker> PartialEq<usize> for AtomicId<T> {
    fn eq(&self, other: &usize) -> bool {
        self.get() == *other
    }

    fn ne(&self, other: &usize) -> bool {
        self.get() != *other
    }
}

impl<T: AtomicIdMarker> PartialOrd<usize> for AtomicId<T> {
    fn lt(&self, other: &usize) -> bool {
        self.get() < *other
    }

    fn le(&self, other: &usize) -> bool {
        self.get() <= *other
    }

    fn gt(&self, other: &usize) -> bool {
        self.get() > *other
    }

    fn ge(&self, other: &usize) -> bool {
        self.get() >= *other
    }

    fn partial_cmp(&self, other: &usize) -> Option<core::cmp::Ordering> {
        self.get().partial_cmp(other)
    }
}

impl<T: AtomicIdMarker> AtomicId<T> {
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        Self(NEXT.fetch_add(1, Ordering::Relaxed), PhantomData)
    }

    pub fn new_val(value: usize) -> Self {
        Self(value, PhantomData)
    }

    pub fn get(&self) -> usize {
        self.0
    }

    fn test() {
        struct IdInner;
        impl AtomicIdMarker for IdInner {}
        type TestId = AtomicId<IdInner>;

        let id1 = TestId::new();
        let b = id1 >= TestId::new_val(1234);
    }
}
