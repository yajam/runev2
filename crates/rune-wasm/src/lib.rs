//! Minimal WASM runtime for executing Rune IR logic modules.
//!
//! This provides a wasmtime-based engine with a small host surface:
//! - rune.core.dispatch_mutation(ptr: i32, len: i32): push JSON mutation to host
//! - rune.assets.fetch(ptr: i32, len: i32): currently a stub gated by `network` capability

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use anyhow::Result;
use rune_ir::{logic::LogicEngine, package::RunePackage};
use tracing::{trace, warn};
use wasmtime::{Caller, Engine, Instance, Linker, Memory, Module, Store};

#[derive(Debug, Clone)]
pub struct WasmMutation {
    pub kind: String,
    pub payload: serde_json::Value,
}

pub trait MutationHandler {
    fn handle_mutation(&mut self, mutation: WasmMutation);
}

#[derive(Default)]
struct HostState {
    handler: Option<Rc<RefCell<dyn MutationHandler>>>,
    capabilities: HashSet<String>,
}

pub struct WasmRuntime {
    engine: Engine,
    linker: Linker<HostState>,
    store: Store<HostState>,
    modules: HashMap<String, Module>,
    instances: HashMap<String, Instance>,
}

#[derive(Debug, thiserror::Error)]
pub enum WasmRuntimeError {
    #[error("module not found: {0}")]
    UnknownModule(String),
    #[error("export not found: {0}")]
    MissingExport(&'static str),
    #[error("runtime error: {0}")]
    Runtime(String),
}

impl WasmRuntime {
    pub fn new() -> Result<Self> {
        let engine = Engine::default();
        let mut linker: Linker<HostState> = Linker::new(&engine);

        // Install host imports under module "rune".
        // Backward-compatible mutation entrypoint names:
        // - core_dispatch_mutation (original)
        // - execute_mutation (alias)
        // - rune_execute_mutation (alias)
        let mut add_dispatch = |name: &str| {
            linker
                .func_wrap(
                    "rune",
                    name,
                    |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| {
                        if let Some((mem, data)) = read_guest_bytes(&mut caller, ptr, len) {
                            match serde_json::from_slice::<serde_json::Value>(&data) {
                                Ok(value) => {
                                    let kind = value
                                        .get("type")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown")
                                        .to_string();
                                    if let Some(h) = caller.data().handler.as_ref()
                                        && let Ok(mut hmut) = h.try_borrow_mut()
                                    {
                                        hmut.handle_mutation(WasmMutation {
                                            kind,
                                            payload: value,
                                        });
                                    }
                                    let _ = mem;
                                }
                                Err(e) => warn!("dispatch_mutation received invalid JSON: {e}"),
                            }
                        }
                    },
                )
                .expect("failed to add mutation dispatch import");
        };
        add_dispatch("core_dispatch_mutation");
        add_dispatch("execute_mutation");
        add_dispatch("rune_execute_mutation");

        linker
            .func_wrap(
                "rune",
                "assets_fetch",
                |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| -> i32 {
                    if !caller.data().capabilities.contains("network") {
                        warn!("assets_fetch called without 'network' capability");
                        return 0;
                    }
                    if let Some((_mem, data)) = read_guest_bytes(&mut caller, ptr, len) {
                        let path = String::from_utf8_lossy(&data);
                        trace!(%path, "assets_fetch invoked (stub)");
                    }
                    0
                },
            )
            .expect("failed to add assets_fetch");

        let store = Store::new(&engine, HostState::default());
        Ok(Self {
            engine,
            linker,
            store,
            modules: HashMap::new(),
            instances: HashMap::new(),
        })
    }

    /// Call an exported function with signature (i32 ptr, i32 len) on any loaded instance,
    /// writing `payload` into guest memory before invocation. Returns true if at least one call succeeded.
    #[allow(clippy::collapsible_if)]
    pub fn call_export_json_any(&mut self, export: &str, payload: &str) -> bool {
        let bytes = payload.as_bytes();
        let mut called = false;
        let instances_keys: Vec<String> = self.instances.keys().cloned().collect();
        for key in instances_keys {
            if let Some(inst) = self.instances.get(&key) {
                if let Some(func) = inst.get_func(&mut self.store, export) {
                    // Obtain memory export
                    if let Some(mem) = inst.get_memory(&mut self.store, "memory") {
                        let needed = bytes.len();
                        let mem_size = mem.data_size(&self.store);
                        // Choose an offset far from static data region (which starts at 0)
                        let offset = 8192usize;
                        if offset + needed <= mem_size
                            && mem.write(&mut self.store, offset, bytes).is_ok()
                        {
                            let ptr = offset as i32;
                            let len = needed as i32;
                            let _ = func.call(
                                &mut self.store,
                                &[wasmtime::Val::I32(ptr), wasmtime::Val::I32(len)],
                                &mut [],
                            );
                            called = true;
                        }
                    }
                }
            }
        }
        called
    }

    pub fn set_mutation_handler(&mut self, handler: Rc<RefCell<dyn MutationHandler>>) {
        self.store.data_mut().handler = Some(handler);
    }

    pub fn set_capabilities<I, S>(&mut self, caps: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let data = self.store.data_mut();
        data.capabilities.clear();
        data.capabilities.extend(caps.into_iter().map(Into::into));
    }

    /// Load WASM modules referenced by a package.
    pub fn register_package_modules(&mut self, package: &RunePackage) {
        for (name, desc) in &package.logic_modules {
            if desc.engine != LogicEngine::Wasm {
                continue;
            }
            let path = package.base_path().join(&desc.module);
            match std::fs::read(&path) {
                Ok(bytes) => {
                    // Try raw bytes first, then fall back to WAT if needed.
                    match Module::new(&self.engine, &bytes) {
                        Ok(module) => {
                            tracing::trace!(module = %name, path = %path.display(), "registered WASM module from package");
                            let _ = self.modules.insert(name.clone(), module);
                        }
                        Err(first_err) => {
                            // Attempt to parse as WAT (common for sample/demo files).
                            match wat::parse_bytes(&bytes) {
                                Ok(compiled) => match Module::new(&self.engine, &compiled) {
                                    Ok(module) => {
                                        tracing::trace!(module = %name, path = %path.display(), "registered WAT module (compiled to WASM)");
                                        let _ = self.modules.insert(name.clone(), module);
                                    }
                                    Err(e) => {
                                        warn!(module = %name, error = %e, "failed to compile WAT->WASM module")
                                    }
                                },
                                Err(_) => {
                                    warn!(module = %name, error = %first_err, "failed to compile WASM module")
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!(module = %name, path = %path.display(), error = %e, "failed to read WASM module from package")
                }
            }
        }
    }

    /// Instantiate a module by name and call `start()` if it exists.
    pub fn execute_module(&mut self, name: &str) -> Result<(), WasmRuntimeError> {
        let module = self
            .modules
            .get(name)
            .ok_or_else(|| WasmRuntimeError::UnknownModule(name.to_string()))?
            .clone();
        let instance = self
            .linker
            .instantiate(&mut self.store, &module)
            .map_err(|e| WasmRuntimeError::Runtime(e.to_string()))?;
        self.instances.insert(name.to_string(), instance);
        if let Some(export) = self
            .instances
            .get(name)
            .and_then(|i| i.get_func(&mut self.store, "start"))
        {
            export
                .call(&mut self.store, &[], &mut [])
                .map_err(|e| WasmRuntimeError::Runtime(e.to_string()))?;
        }
        Ok(())
    }

    /// Invoke `tick()` if the active instance for this module provides it.
    pub fn tick(&mut self, name: &str) -> Result<(), WasmRuntimeError> {
        let Some(inst) = self.instances.get(name) else {
            return Ok(());
        };
        if let Some(func) = inst.get_func(&mut self.store, "tick") {
            func.call(&mut self.store, &[], &mut [])
                .map_err(|e| WasmRuntimeError::Runtime(e.to_string()))?;
        }
        Ok(())
    }
}

fn read_guest_bytes(
    caller: &mut Caller<'_, HostState>,
    ptr: i32,
    len: i32,
) -> Option<(Memory, Vec<u8>)> {
    let export = caller.get_export("memory")?;
    let mem = export.into_memory()?;
    let mut buf = vec![0u8; len as usize];
    if mem.read(caller, ptr as usize, &mut buf).is_ok() {
        Some((mem, buf))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[derive(Default)]
    struct Recorder(Rc<RefCell<Vec<WasmMutation>>>);
    impl MutationHandler for Recorder {
        fn handle_mutation(&mut self, mutation: WasmMutation) {
            self.0.borrow_mut().push(mutation);
        }
    }

    #[test]
    fn invokes_start_and_dispatches_mutation() {
        // A tiny module that calls the host dispatch with a JSON string.
        // (module
        //   (import "rune" "core_dispatch_mutation" (func $dispatch (param i32 i32)))
        //   (memory (export "memory") 1)
        //   (data (i32.const 0) "{\"type\":\"noop\",\"x\":1}")
        //   (func (export "start")
        //     (call $dispatch (i32.const 0) (i32.const 22))))
        let wat = r#"
            (module
                (import "rune" "core_dispatch_mutation" (func $dispatch (param i32 i32)))
                (memory (export "memory") 1)
                (data (i32.const 0) "{\"type\":\"noop\",\"x\":1}")
                (func (export "start")
                    (call $dispatch (i32.const 0) (i32.const 21))))
        "#;
        let wasm = wat::parse_str(wat).expect("WAT should parse");

        let mut rt = WasmRuntime::new().expect("runtime should construct");
        let rec = Recorder::default();
        let sink = rec.0.clone();
        rt.set_mutation_handler(Rc::new(RefCell::new(rec)));

        // Register an ad-hoc module in-memory (simulate package)
        let module = Module::new(&rt.engine, &wasm).expect("compile module");
        rt.modules.insert("test".to_string(), module);

        rt.execute_module("test").expect("start should run");

        let list = sink.borrow();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].kind, "noop");
        assert_eq!(list[0].payload["x"], 1);
    }
}
