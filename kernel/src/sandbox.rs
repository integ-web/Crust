use anyhow::{Result, bail};
use wasmtime::{Config, Engine, Store, Module, Instance};
use std::time::Duration;
use tracing::warn;

pub struct WasmSandbox {
    engine: Engine,
}

impl WasmSandbox {
    pub fn new() -> Result<Self> {
        let mut config = Config::new();
        // Enable epoch-based interruption (time deadlines)
        config.epoch_interruption(true);
        // Enable fuel consumption (instruction count limits)
        config.consume_fuel(true);

        let engine = Engine::new(&config)?;
        Ok(Self { engine })
    }

    /// Executes a WASM module with strictly metered resources.
    /// - `max_fuel`: The maximum instruction fuel.
    /// - `timeout`: The epoch deadline duration.
    pub fn execute_metered(&self, wasm_bytes: &[u8], max_fuel: u64, timeout: Duration) -> Result<i32> {
        let module = Module::new(&self.engine, wasm_bytes)?;
        let mut store = Store::new(&self.engine, ());

        // 1. Metering: Set the initial fuel limit
        store.set_fuel(max_fuel)?;

        // 2. Epoch Interruption: Setup a background thread to bump the engine's epoch counter
        // This is a naive implementation just for the example. In a real OS, a single global timer thread
        // bumps the engine epoch every ~100ms, and we set `store.set_epoch_deadline(current + X)`
        store.set_epoch_deadline(1);
        let engine_clone = self.engine.clone();

        let _timer_thread = std::thread::spawn(move || {
            std::thread::sleep(timeout);
            engine_clone.increment_epoch();
        });

        // Instantiate and run
        let instance = match Instance::new(&mut store, &module, &[]) {
            Ok(inst) => inst,
            Err(e) => {
                bail!("Failed to instantiate module: {}", e);
            }
        };

        // Assume the module exports a function called "run" that returns an i32
        let run_func = instance.get_typed_func::<(), i32>(&mut store, "run")?;

        let result = match run_func.call(&mut store, ()) {
            Ok(val) => val,
            Err(e) => {
                if let Some(_) = e.downcast_ref::<wasmtime::Trap>() {
                    warn!("WASM Execution Trapped! (Likely out of fuel or time deadline exceeded): {}", e);
                } else {
                    warn!("WASM Execution Failed: {}", e);
                }
                bail!("Execution interrupted or failed: {}", e);
            }
        };

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuel_consumption() -> Result<()> {
        let sandbox = WasmSandbox::new()?;

        // A simple WASM module in WAT format that loops infinitely
        let wat = r#"
            (module
                (func $run (export "run") (result i32)
                    (loop $my_loop
                        br $my_loop
                    )
                    i32.const 42
                )
            )
        "#;
        let wasm_bytes = wat::parse_str(wat)?;

        // Set fuel very low (e.g., 100 instructions), time very high (10 seconds)
        // It should trap out of fuel before timing out.
        let result = sandbox.execute_metered(&wasm_bytes, 100, Duration::from_secs(10));

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("out of fuel") || err_msg.contains("Execution interrupted"));

        Ok(())
    }

    #[test]
    fn test_successful_execution() -> Result<()> {
        let sandbox = WasmSandbox::new()?;

        // A simple WASM module that just returns 42
        let wat = r#"
            (module
                (func $run (export "run") (result i32)
                    i32.const 42
                )
            )
        "#;
        let wasm_bytes = wat::parse_str(wat)?;

        // Give it plenty of fuel and time
        let result = sandbox.execute_metered(&wasm_bytes, 10000, Duration::from_secs(2))?;
        assert_eq!(result, 42);

        Ok(())
    }
}
