//! Isolated qualification host. Uploaded data never supplies a WASM module.
use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    sync::{mpsc, Mutex},
    time::{Duration, Instant},
};
use wasmtime::{Config, Engine, Instance, Module, Store, StoreLimits, StoreLimitsBuilder, Trap};

pub mod job;

pub fn sha(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
#[derive(Clone)]
pub struct Limits {
    pub memory_bytes: usize,
    pub fuel: u64,
    pub timeout: Duration,
    pub max_input_bytes: usize,
    pub max_output_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            memory_bytes: 2 * 1024 * 1024 * 1024,
            fuel: 5_000_000_000,
            timeout: Duration::from_secs(120),
            max_input_bytes: 100 * 1024 * 1024,
            max_output_bytes: 128 * 1024 * 1024,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct RunReport {
    pub result: Value,
    pub elapsed_ms: u64,
    pub memory_bytes: usize,
    pub fuel_consumed: u64,
    pub wasm_sha256: String,
    pub interface_imports: Vec<String>,
    pub resource_limits: Value,
    pub input_sha256: String,
    pub config_sha256: String,
}
struct State {
    limits: StoreLimits,
}
pub struct Sandbox {
    engine: Engine,
    module: Module,
    wasm_sha256: String,
    serial: Mutex<()>,
}
impl Sandbox {
    pub fn new(bytes: &[u8]) -> Result<Self> {
        let mut config = Config::new();
        config
            .consume_fuel(true)
            .epoch_interruption(true)
            .max_wasm_stack(512 * 1024);
        let engine = Engine::new(&config)?;
        let module = Module::new(&engine, bytes)?;
        if let Some(import) = module.imports().next() {
            bail!("forbidden_import: {}::{}", import.module(), import.name());
        }
        Ok(Self {
            engine,
            module,
            wasm_sha256: sha(bytes),
            serial: Mutex::new(()),
        })
    }
    pub fn run(&self, input: &[u8], config: &Value, limits: Limits) -> Result<RunReport> {
        self.run_with_decoder(input, config, limits, |bytes| {
            serde_json::from_slice(bytes).context("guest_output_invalid")
        })
    }
    fn run_with_decoder(
        &self,
        input: &[u8],
        config: &Value,
        limits: Limits,
        decode: impl FnOnce(&[u8]) -> Result<Value>,
    ) -> Result<RunReport> {
        // One engine's epochs affect every store; serialize this spike's instances.
        // The integrated worker must use a per-job engine or coordinated deadlines.
        let _guard = self
            .serial
            .lock()
            .map_err(|_| anyhow::anyhow!("host_mutex"))?;
        let config_bytes = serde_json::to_vec(config)?;
        if config_bytes.len() > 4096 {
            bail!("config_limit");
        }
        let cap = match (config["profile"].as_str(), config["format"].as_str()) {
            (Some("demo"), Some("csv")) => 10 * 1024 * 1024,
            (Some("demo"), Some("xlsx")) => 5 * 1024 * 1024,
            (Some("platform"), Some("csv")) => 100 * 1024 * 1024,
            (Some("platform"), Some("xlsx")) => 10 * 1024 * 1024,
            _ => bail!("profile_or_format_invalid"),
        };
        if input.len() > limits.max_input_bytes.min(cap) {
            bail!("input_limit");
        }
        let ceiling = Limits::default();
        if limits.memory_bytes > ceiling.memory_bytes
            || limits.timeout > ceiling.timeout
            || limits.fuel > ceiling.fuel
            || limits.max_input_bytes > ceiling.max_input_bytes
            || limits.max_output_bytes > ceiling.max_output_bytes
        {
            bail!("runtime_limit_may_not_increase");
        }
        // Include native setup and decoding in the acceptance deadline. Epochs
        // interrupt WASM; the final fence rejects late native completion too.
        let started = Instant::now();
        let limiter = StoreLimitsBuilder::new()
            .memory_size(limits.memory_bytes)
            .instances(1)
            .memories(1)
            .tables(1)
            .table_elements(10000)
            .trap_on_grow_failure(true)
            .build();
        let mut store = Store::new(&self.engine, State { limits: limiter });
        store.limiter(|s| &mut s.limits);
        store.set_fuel(limits.fuel)?;
        store.set_epoch_deadline(1);
        store.epoch_deadline_trap();
        let (cancel_tx, cancel_rx) = mpsc::channel();
        let engine = self.engine.clone();
        let timeout = limits.timeout;
        let timer = std::thread::spawn(move || {
            if cancel_rx.recv_timeout(timeout.saturating_sub(started.elapsed()))
                == Err(mpsc::RecvTimeoutError::Timeout)
            {
                engine.increment_epoch();
            }
        });
        let mut memory = None;
        let result = (|| -> Result<Value> {
            let instance = Instance::new(&mut store, &self.module, &[])?;
            let mem = instance
                .get_memory(&mut store, "memory")
                .context("guest_memory_missing")?;
            memory = Some(mem);
            let alloc = instance.get_typed_func::<u32, u32>(&mut store, "alloc")?;
            let input_ptr = alloc.call(&mut store, input.len() as u32)?;
            mem.write(&mut store, input_ptr as usize, input)?;
            let config_ptr = alloc.call(&mut store, config_bytes.len() as u32)?;
            mem.write(&mut store, config_ptr as usize, &config_bytes)?;
            let parse =
                instance.get_typed_func::<(u32, u32, u32, u32), u32>(&mut store, "parse")?;
            let status = parse.call(
                &mut store,
                (
                    input_ptr,
                    input.len() as u32,
                    config_ptr,
                    config_bytes.len() as u32,
                ),
            )?;
            let ptr = instance
                .get_typed_func::<(), u32>(&mut store, "result_ptr")?
                .call(&mut store, ())?;
            let len = instance
                .get_typed_func::<(), u32>(&mut store, "result_len")?
                .call(&mut store, ())? as usize;
            if len > limits.max_output_bytes {
                bail!("output_limit");
            }
            let end = (ptr as usize).checked_add(len).context("output_bounds")?;
            let bytes = mem
                .data(&store)
                .get(ptr as usize..end)
                .context("output_bounds")?;
            let value: Value = decode(bytes)?;
            if (status == 0) != (value["status"] == "parsed") {
                bail!("guest_status_mismatch");
            }
            Ok(value)
        })();
        let _ = cancel_tx.send(());
        timer.join().map_err(|_| anyhow::anyhow!("timer_failed"))?;
        let input_sha256 = sha(input);
        let config_sha256 = sha(&config_bytes);
        let elapsed = started.elapsed();
        let elapsed_ms = elapsed.as_millis() as u64;
        let result = match result {
            Ok(v) if v["status"] == "parsed" && elapsed >= limits.timeout => {
                json!({"status":"refused","reason":"time_limit","no_revision_created":true})
            }
            Ok(v) => v,
            Err(e) => json!({"status":"refused","reason":classify(&e),"no_revision_created":true}),
        };
        Ok(RunReport {
            result,
            elapsed_ms,
            memory_bytes: memory.map_or(0, |m| m.data_size(&store)),
            fuel_consumed: limits.fuel - store.get_fuel()?,
            wasm_sha256: self.wasm_sha256.clone(),
            interface_imports: self
                .module
                .imports()
                .map(|i| format!("{}::{}", i.module(), i.name()))
                .collect(),
            resource_limits: json!({"guest_linear_memory_bytes":limits.memory_bytes,"fuel":limits.fuel,"wall_clock_ms":limits.timeout.as_millis(),"input_bytes":limits.max_input_bytes.min(cap),"output_bytes":limits.max_output_bytes}),
            input_sha256,
            config_sha256,
        })
    }
}
fn classify(error: &anyhow::Error) -> &'static str {
    if let Some(trap) = error.downcast_ref::<Trap>() {
        return match trap {
            Trap::OutOfFuel => "fuel_exhausted",
            Trap::Interrupt => "time_limit",
            Trap::StackOverflow => "stack_limit",
            Trap::MemoryOutOfBounds => "guest_memory_access",
            _ => "guest_trap",
        };
    }
    let msg = format!("{error:#}").to_ascii_lowercase();
    if msg.contains("memory") || msg.contains("grow") {
        "memory_limit"
    } else if msg.contains("output_limit") {
        "output_limit"
    } else {
        "guest_failure"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_decode_finishing_after_deadline_cannot_return_success() {
        // The guest has already returned valid JSON before this deliberately
        // delayed native decoder runs. Epoch checks cannot interrupt this code.
        let module = br#"(module
          (memory (export "memory") 1)
          (data (i32.const 0) "{\22status\22:\22parsed\22}")
          (func (export "alloc") (param i32) (result i32) i32.const 100)
          (func (export "parse") (param i32 i32 i32 i32) (result i32) i32.const 0)
          (func (export "result_ptr") (result i32) i32.const 0)
          (func (export "result_len") (result i32) i32.const 19))"#;
        let sandbox = Sandbox::new(module).unwrap();
        let config = json!({"profile":"demo","format":"csv"});
        let mut decoded = false;
        let report = sandbox
            .run_with_decoder(
                b"",
                &config,
                Limits {
                    timeout: Duration::from_millis(100),
                    ..Limits::default()
                },
                |bytes| {
                    let value = serde_json::from_slice(bytes)?;
                    decoded = true;
                    std::thread::sleep(Duration::from_millis(120));
                    Ok(value)
                },
            )
            .unwrap();
        assert!(
            decoded,
            "control must reach native decoding after valid WASM output"
        );
        assert_eq!(report.result["reason"], "time_limit");
        assert_eq!(report.result["no_revision_created"], true);
        assert!(report.elapsed_ms >= 120);
        assert_eq!(
            sandbox.run(b"", &config, Limits::default()).unwrap().result["status"],
            "parsed"
        );
    }
}
