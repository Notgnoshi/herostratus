//! Observers extract facts from commits and emit typed observations.
mod commit_context;
mod observation;
#[allow(clippy::module_inception)]
mod observer;
mod observer_data;
mod observer_engine;
mod observer_factory;

mod impls;

pub use commit_context::CommitContext;
pub use observation::Observation;
pub use observer::DiffAction;
#[expect(unused)]
pub(crate) use observer_data::ObserverData;
