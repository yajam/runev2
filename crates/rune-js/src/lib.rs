//! JavaScript runtime integration facade.

#![allow(clippy::all)]

use std::error::Error;
use std::fmt;

/// Errors that can be raised when interacting with the JavaScript runtime facade.
#[derive(Debug)]
pub enum JsRuntimeError {
    /// Raised when the JavaScript runtime feature is not available in this build.
    FeatureUnavailable,
    #[cfg(feature = "boa")]
    /// Raised when the underlying Boa context could not be created.
    ContextCreation(boa_engine::JsError),
    #[cfg(feature = "boa")]
    /// Raised when executing user code produced a JavaScript exception.
    ExecutionError(String),
}

impl fmt::Display for JsRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsRuntimeError::FeatureUnavailable => {
                write!(
                    f,
                    "JavaScript runtime is unavailable (enable the `boa` feature)"
                )
            }
            #[cfg(feature = "boa")]
            JsRuntimeError::ContextCreation(err) => {
                write!(f, "failed to create JavaScript context: {err}")
            }
            #[cfg(feature = "boa")]
            JsRuntimeError::ExecutionError(msg) => write!(f, "JavaScript error: {msg}"),
        }
    }
}

impl Error for JsRuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            JsRuntimeError::FeatureUnavailable => None,
            #[cfg(feature = "boa")]
            JsRuntimeError::ContextCreation(err) => Some(err),
            #[cfg(feature = "boa")]
            JsRuntimeError::ExecutionError(_) => None,
        }
    }
}

#[cfg(feature = "boa")]
mod boa_runtime {
    use std::{
        cell::RefCell,
        collections::{HashMap, HashSet},
        rc::Rc,
    };

    use super::JsRuntimeError;
    use boa_engine::{
        Context, JsArgs, JsError, JsNativeError, JsResult, JsValue, Source, js_string,
        native_function::NativeFunction, object::ObjectInitializer, property::Attribute,
    };
    use boa_gc::{Finalize, Trace};
    use rune_ir::package::RunePackage;
    use tracing::{error, trace, warn};

    /// Mutation object emitted from JavaScript, expressed as JSON for now.
    /// This keeps the bridge stable while the full IrMutation interface is evolving.
    #[derive(Debug, Clone)]
    pub struct JsMutation {
        pub kind: String,
        pub payload: serde_json::Value,
    }

    /// Trait implemented by the host to receive mutations from JS.
    pub trait MutationHandler {
        fn handle_mutation(&mut self, mutation: JsMutation);
    }

    /// Host cells shared with native closures.
    #[derive(Trace, Finalize, Clone)]
    struct HostCells {
        #[unsafe_ignore_trace]
        handler: Rc<RefCell<Option<Rc<RefCell<dyn MutationHandler>>>>>,
        #[unsafe_ignore_trace]
        capabilities: Rc<RefCell<HashSet<String>>>,
    }

    /// Wrapper around Boa's JavaScript execution context.
    pub struct JsRuntime {
        context: Context,
        modules: HashMap<String, String>,
        host_cells: HostCells,
        timers_installed: bool,
    }

    impl JsRuntime {
        /// Constructs a new JavaScript runtime backed by Boa.
        pub fn new() -> Result<Self, JsRuntimeError> {
            let context = Context::builder()
                .build()
                .map_err(JsRuntimeError::ContextCreation)?;
            let mut runtime = Self {
                context,
                modules: HashMap::new(),
                host_cells: HostCells {
                    handler: Rc::new(RefCell::new(None)),
                    capabilities: Rc::new(RefCell::new(HashSet::new())),
                },
                timers_installed: false,
            };
            runtime.install_host_bindings();
            Ok(runtime)
        }

        /// Sets the mutation handler that will receive mutations from JS.
        pub fn set_mutation_handler(&mut self, handler: Rc<RefCell<dyn MutationHandler>>) {
            *self.host_cells.handler.borrow_mut() = Some(handler);
        }

        /// Configure allowed capabilities for host APIs.
        pub fn set_capabilities<I, S>(&mut self, caps: I)
        where
            I: IntoIterator<Item = S>,
            S: Into<String>,
        {
            let mut set = self.host_cells.capabilities.borrow_mut();
            set.clear();
            set.extend(caps.into_iter().map(Into::into));
        }

        /// Register a module by name with its source code.
        pub fn register_module(&mut self, name: impl Into<String>, source: impl Into<String>) {
            self.modules.insert(name.into(), source.into());
        }

        /// Load modules referenced by a package (if present on disk via base_path).
        pub fn register_package_modules(&mut self, package: &RunePackage) {
            for (name, desc) in &package.logic_modules {
                let path = package.base_path().join(&desc.module);
                match std::fs::read_to_string(&path) {
                    Ok(source) => {
                        trace!(module = %name, path = %path.display(), "registering JS module from package");
                        self.modules.insert(name.clone(), source);
                    }
                    Err(e) => {
                        warn!(module = %name, path = %path.display(), error = %e, "failed to read JS module from package");
                    }
                }
                // record capabilities into host cells
                let mut set = self.host_cells.capabilities.borrow_mut();
                for cap in &desc.capabilities {
                    set.insert(cap.clone());
                }
            }
        }

        /// Executes an arbitrary JavaScript snippet and returns the resulting value.
        pub fn execute_script(&mut self, script: &str) -> JsResult<JsValue> {
            let source = Source::from_bytes(script);
            self.context.eval(source)
        }

        /// Executes a previously registered module by name.
        pub fn execute_module(&mut self, name: &str) -> Result<JsValue, JsRuntimeError> {
            let Some(source) = self.modules.get(name) else {
                return Err(JsRuntimeError::ExecutionError(format!(
                    "unknown module '{name}'"
                )));
            };
            match self.context.eval(Source::from_bytes(source.as_str())) {
                Ok(v) => Ok(v),
                Err(e) => Err(JsRuntimeError::ExecutionError(format!("{e}"))),
            }
        }

        /// Drive any pending timers and microtasks; call regularly from the host loop.
        pub fn pump_tasks(&mut self) {
            if !self.timers_installed {
                let _ = self.install_js_queue_scaffolding();
            }
            if let Err(err) = self.context.eval(Source::from_bytes(
                "if (globalThis.__rune && __rune.pump) { __rune.pump(); }",
            )) {
                error!(%err, "error while pumping JS tasks");
            }
        }

        /// Provides mutable access to the underlying Boa context for advanced usage.
        pub fn context_mut(&mut self) -> &mut Context {
            &mut self.context
        }

        fn install_host_bindings(&mut self) {
            // Install timer/microtask queues and pump, then host namespaces.
            if let Err(err) = self.install_js_queue_scaffolding() {
                error!(%err, "failed to install JS queue scaffolding");
            }
            self.install_rune_namespace();
        }

        fn install_js_queue_scaffolding(&mut self) -> JsResult<JsValue> {
            let script = r#"
                (function(){
                    const g = globalThis;
                    if (!g.__rune) g.__rune = {};
                    const R = g.__rune;
                    if (!R.__installed) {
                        R.timerId = 0;
                        R.timers = [];
                        R.microtasks = [];
                        R.enqueueTimer = function(fn, args){
                            const id = ++R.timerId >>> 0;
                            R.timers.push({ id, fn, args });
                            return id;
                        };
                        R.cancelTimer = function(id){
                            const idx = R.timers.findIndex(t => t.id === id);
                            if (idx >= 0) { R.timers.splice(idx, 1); }
                        };
                        R.enqueueMicrotask = function(fn){
                            if (typeof fn !== 'function') return;
                            R.microtasks.push(fn);
                        };
                        R.pump = function(){
                            // microtasks first
                            while (R.microtasks.length) {
                                const batch = R.microtasks.splice(0);
                                for (const cb of batch) {
                                    try { cb(); } catch (e) { /* TODO: error surfacing */ }
                                }
                            }
                            // then timers queued so far
                            if (R.timers.length) {
                                const timers = R.timers.splice(0);
                                for (const t of timers) {
                                    try { t.fn.apply(undefined, t.args || []); } catch (e) { /* ignore */ }
                                }
                            }
                            // drain any microtasks scheduled by timers
                            while (R.microtasks.length) {
                                const batch = R.microtasks.splice(0);
                                for (const cb of batch) {
                                    try { cb(); } catch (e) {}
                                }
                            }
                        };
                        // global shims
                        g.setTimeout = function(fn, ms){
                            const args = Array.prototype.slice.call(arguments, 2);
                            return R.enqueueTimer(fn, args);
                        };
                        g.clearTimeout = function(id){ R.cancelTimer(id); };
                        g.queueMicrotask = function(fn){ R.enqueueMicrotask(fn); };
                        R.__installed = true;
                    }
                })();
            "#;
            let result = self.context.eval(Source::from_bytes(script));
            if result.is_ok() {
                self.timers_installed = true;
            }
            result
        }

        fn install_rune_namespace(&mut self) {
            // Prepare captures for dispatch + assets.fetch
            let host_cap = self.host_cells.clone();

            // core object
            let core_obj = ObjectInitializer::new(&mut self.context)
                .function(
                    NativeFunction::from_copy_closure_with_captures(
                        |_, args, cap: &HostCells, context| {
                            let json_val = args.get_or_undefined(0);
                            let js_str = json_val.to_string(context)?;
                            let json_str: String = js_str.to_std_string_escaped();
                            match serde_json::from_str::<serde_json::Value>(&json_str) {
                                Ok(value) => {
                                    let kind = value
                                        .get("type")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown")
                                        .to_string();
                                    if let Some(handler) = cap.handler.borrow().as_ref() {
                                        if let Ok(mut h) = handler.try_borrow_mut() {
                                            h.handle_mutation(JsMutation {
                                                kind,
                                                payload: value,
                                            });
                                        }
                                    }
                                    Ok(JsValue::undefined())
                                }
                                Err(e) => Err(JsError::from_native(
                                    JsNativeError::typ().with_message(format!(
                                        "dispatchMutation: expected JSON string, got error: {e}"
                                    )),
                                )),
                            }
                        },
                        host_cap.clone(),
                    ),
                    (
                        js_string!("dispatchMutation"),
                        js_string!("dispatchMutation"),
                    ),
                    1,
                )
                .build();

            // assets object
            let assets_obj = ObjectInitializer::new(&mut self.context)
                .function(
                    NativeFunction::from_copy_closure_with_captures(
                        |_, args, cap: &HostCells, context| {
                            let path = args
                                .get_or_undefined(0)
                                .to_string(context)?
                                .to_std_string_escaped();
                            if !cap.capabilities.borrow().contains("network") {
                                return Err(JsError::from_native(
                                    JsNativeError::typ().with_message(
                                        "assets.fetch: 'network' capability not granted",
                                    ),
                                ));
                            }
                            trace!(%path, "assets.fetch invoked (stub)");
                            Ok(JsValue::undefined())
                        },
                        host_cap,
                    ),
                    (js_string!("fetch"), js_string!("fetch")),
                    1,
                )
                .build();

            // rune object with namespaces
            let rune_obj = ObjectInitializer::new(&mut self.context)
                .property(js_string!("core"), core_obj, Attribute::all())
                .property(js_string!("assets"), assets_obj, Attribute::all())
                .build();
            let _ = self.context.register_global_property(
                js_string!("rune"),
                rune_obj,
                Attribute::all(),
            );

            // Install a small JS helper for HTTP promises that uses dispatchMutation
            // and a host callback name to resolve the promise when the host replies.
            let http_js = r#"
                (function(){
                  const g = globalThis;
                  if (!g.rune) g.rune = {};
                  const R = g.rune;
                  if (!R.http) R.http = {};
                  if (!R.http._token) { R.http._token = 0; R.http._callbacks = {}; }
                  if (!R.http.get) R.http.get = function(url){
                    const token = (++R.http._token).toString(36);
                    const cbName = "__rune_http_cb_" + token;
                    return new Promise((resolve, reject) => {
                      R.http._callbacks[cbName] = function(payload){
                        try {
                          if (typeof payload === 'string') { resolve(JSON.parse(payload)); }
                          else { resolve(payload); }
                        } catch (e) { resolve(payload); }
                        delete R.http._callbacks[cbName];
                        try { delete g[cbName]; } catch(_){}
                      };
                      g[cbName] = R.http._callbacks[cbName];
                      if (R.core && typeof R.core.dispatchMutation === 'function') {
                        R.core.dispatchMutation(JSON.stringify({ type: 'http_fetch', url: String(url), method: 'get', callback: cbName }));
                      }
                    });
                  };
                })();
            "#;
            let _ = self.context.eval(Source::from_bytes(http_js));
        }
    }

    impl Default for JsRuntime {
        fn default() -> Self {
            Self::new().expect("failed to construct Boa JavaScript runtime")
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{JsMutation, JsRuntime, MutationHandler};
        use std::{cell::RefCell, rc::Rc};

        #[derive(Default)]
        struct Recorder(Rc<RefCell<Vec<JsMutation>>>);

        impl MutationHandler for Recorder {
            fn handle_mutation(&mut self, mutation: JsMutation) {
                self.0.borrow_mut().push(mutation);
            }
        }

        #[test]
        fn executes_basic_script() {
            let mut runtime = JsRuntime::new().expect("runtime should construct");
            let value = runtime
                .execute_script("1 + 2")
                .expect("script should execute");
            assert_eq!(value.as_number(), Some(3.0));
        }

        #[test]
        fn dispatches_mutation_via_host() {
            let mut runtime = JsRuntime::new().expect("runtime should construct");
            let rec = Recorder::default();
            let sink = rec.0.clone();
            runtime.set_mutation_handler(Rc::new(RefCell::new(rec)));

            runtime
                .execute_script(
                    r#"
                rune.core.dispatchMutation('{"type":"noop","x":1}');
            "#,
                )
                .expect("dispatch should work");

            let list = sink.borrow();
            assert_eq!(list.len(), 1);
            assert_eq!(list[0].kind, "noop");
            assert_eq!(list[0].payload["x"], 1);
        }

        #[test]
        fn timers_fire_via_pump() {
            let mut runtime = JsRuntime::new().expect("runtime should construct");
            runtime
                .execute_script(
                    r#"
                var fired = 0;
                setTimeout(function(){ fired = fired + 1; }, 0);
            "#,
                )
                .expect("setTimeout should schedule");
            // No automatic execution; pump to run callbacks.
            runtime.pump_tasks();
            let v = runtime.execute_script("fired").expect("read fired");
            assert_eq!(v.as_number(), Some(1.0));
        }
    }
}

#[cfg(feature = "boa")]
pub use boa_runtime::JsMutation;
#[cfg(feature = "boa")]
pub use boa_runtime::JsRuntime;
#[cfg(feature = "boa")]
pub use boa_runtime::MutationHandler;

#[cfg(not(feature = "boa"))]
#[derive(Debug, Clone)]
pub struct JsMutation {
    pub kind: String,
    pub payload: serde_json::Value,
}

#[cfg(not(feature = "boa"))]
pub trait MutationHandler {
    fn handle_mutation(&mut self, _mutation: JsMutation) {}
}

#[cfg(not(feature = "boa"))]
pub struct JsRuntime;

#[cfg(not(feature = "boa"))]
impl JsRuntime {
    pub fn new() -> Result<Self, JsRuntimeError> {
        Err(JsRuntimeError::FeatureUnavailable)
    }

    pub fn set_mutation_handler(
        &mut self,
        _handler: std::rc::Rc<std::cell::RefCell<dyn MutationHandler>>,
    ) {
    }

    pub fn set_capabilities<I, S>(&mut self, _caps: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
    }

    pub fn register_module(&mut self, _name: impl Into<String>, _source: impl Into<String>) {}
    pub fn register_package_modules(&mut self, _package: &rune_ir::package::RunePackage) {}
    pub fn execute_script(&mut self, _script: &str) -> Result<(), JsRuntimeError> {
        Ok(())
    }
    pub fn execute_module(&mut self, _name: &str) -> Result<(), JsRuntimeError> {
        Ok(())
    }
    pub fn pump_tasks(&mut self) {}
}
