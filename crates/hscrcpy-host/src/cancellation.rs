pub trait CancellationToken {
    fn is_cancelled(&self) -> bool;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoCancellation;

impl CancellationToken for NoCancellation {
    fn is_cancelled(&self) -> bool {
        false
    }
}

impl<T> CancellationToken for &T
where
    T: CancellationToken + ?Sized,
{
    fn is_cancelled(&self) -> bool {
        (*self).is_cancelled()
    }
}
