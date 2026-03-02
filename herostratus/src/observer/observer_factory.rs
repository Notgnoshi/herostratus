use super::observer::Observer;

/// A factory to build [Observer]s.
///
/// Each observer registers an [ObserverFactory] via [inventory::submit!]. Unlike rules,
/// observers have no configuration -- they extract raw facts, and thresholds live in rules.
pub struct ObserverFactory {
    factory: fn() -> Box<dyn Observer>,
}

impl ObserverFactory {
    /// Create an [ObserverFactory] that uses [Default] to build an [Observer].
    pub const fn new<O: Observer + Default + 'static>() -> Self {
        fn create<O: Observer + Default + 'static>() -> Box<dyn Observer> {
            Box::new(O::default())
        }
        Self {
            factory: create::<O>,
        }
    }

    /// Use the factory to build the [Observer].
    pub fn build(&self) -> Box<dyn Observer> {
        (self.factory)()
    }
}

inventory::collect!(ObserverFactory);

/// Get a new instance of each registered [Observer].
#[cfg_attr(not(test), expect(unused))]
pub fn builtin_observers() -> Vec<Box<dyn Observer>> {
    inventory::iter::<ObserverFactory>
        .into_iter()
        .map(|f| f.build())
        .collect()
}
