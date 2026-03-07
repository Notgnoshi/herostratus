//! Observers extract facts from commits and emit typed observations.
mod commit_context;
mod observation;
#[allow(clippy::module_inception)]
mod observer;
mod observer_data;
mod observer_engine;
mod observer_factory;

mod impls;

#[cfg(test)]
mod test_observers;

pub use commit_context::CommitContext;
pub use observation::Observation;
pub use observer::{DiffAction, Observer};
pub use observer_data::ObserverData;
pub use observer_engine::ObserverEngine;
pub use observer_factory::{ObserverFactory, builtin_observers};
