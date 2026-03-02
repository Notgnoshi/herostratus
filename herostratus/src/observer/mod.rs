//! Observers extract facts from commits and emit typed observations.
mod commit_context;
mod observation;
#[allow(clippy::module_inception)]
mod observer;
mod observer_data;
mod observer_engine;
pub(crate) mod observer_factory;

pub(crate) mod impls;

#[cfg(test)]
pub(crate) mod test_observers;

pub use commit_context::CommitContext;
pub use observation::Observation;
pub use observer::{DiffAction, Observer};
#[expect(unused)]
pub(crate) use observer_data::ObserverData;
pub use observer_factory::ObserverFactory;
