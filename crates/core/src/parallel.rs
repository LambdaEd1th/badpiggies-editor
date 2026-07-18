#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;

pub(crate) fn map<T, R, F>(items: Vec<T>, operation: F) -> Vec<R>
where
    T: Send,
    R: Send,
    F: Fn(T) -> R + Send + Sync,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        items.into_par_iter().map(operation).collect()
    }
    #[cfg(target_arch = "wasm32")]
    {
        items.into_iter().map(operation).collect()
    }
}

pub(crate) fn filter_map<T, R, F>(items: Vec<T>, operation: F) -> Vec<R>
where
    T: Send,
    R: Send,
    F: Fn(T) -> Option<R> + Send + Sync,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        items.into_par_iter().filter_map(operation).collect()
    }
    #[cfg(target_arch = "wasm32")]
    {
        items.into_iter().filter_map(operation).collect()
    }
}

pub(crate) fn try_map<T, R, E, F>(items: Vec<T>, operation: F) -> Result<Vec<R>, E>
where
    T: Send,
    R: Send,
    E: Send,
    F: Fn(T) -> Result<R, E> + Send + Sync,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        items.into_par_iter().map(operation).collect()
    }
    #[cfg(target_arch = "wasm32")]
    {
        items.into_iter().map(operation).collect()
    }
}

pub(crate) fn map_ref<T, R, F>(items: &[T], operation: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Send + Sync,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        items.par_iter().map(operation).collect()
    }
    #[cfg(target_arch = "wasm32")]
    {
        items.iter().map(operation).collect()
    }
}

pub(crate) fn try_map_ref<T, R, E, F>(items: &[T], operation: F) -> Result<Vec<R>, E>
where
    T: Sync,
    R: Send,
    E: Send,
    F: Fn(&T) -> Result<R, E> + Send + Sync,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        items.par_iter().map(operation).collect()
    }
    #[cfg(target_arch = "wasm32")]
    {
        items.iter().map(operation).collect()
    }
}

pub(crate) fn join<A, B, RA, RB>(left: A, right: B) -> (RA, RB)
where
    A: FnOnce() -> RA + Send,
    B: FnOnce() -> RB + Send,
    RA: Send,
    RB: Send,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        rayon::join(left, right)
    }
    #[cfg(target_arch = "wasm32")]
    {
        (left(), right())
    }
}
