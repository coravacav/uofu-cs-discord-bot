use std::fmt::Debug;

// This trait should be usable on all iterator chains, notably, any Result (eyre)
pub trait ForwardRefToTracing<T, E> {
    fn trace_err(self) -> Result<T, E>;
    fn trace_err_ok(self) -> Option<T>;
}

impl<T, E> ForwardRefToTracing<T, E> for Result<T, E>
where
    E: Debug,
{
    fn trace_err(self) -> Result<T, E> {
        self.map_err(|e| {
            tracing::error!("{:?}", e);
            e
        })
    }

    fn trace_err_ok(self) -> Option<T> {
        self.map_err(|e| {
            tracing::error!("{:?}", e);
            e
        })
        .ok()
    }
}
