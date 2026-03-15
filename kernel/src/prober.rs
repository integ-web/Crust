use sysinfo::System;
use nvml_wrapper::Nvml;
use tracing::warn;

#[derive(Debug, Clone, PartialEq)]
pub enum HardwareBackend {
    Cuda,
    Metal,
    Sycl,
    Rocm,
    Npu,
    CpuX86,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RunMode {
    FullContext,
    HalfContext,
    CpuOffload,
    CpuOnly,
    MoeExpertSwitch,
}

#[derive(Debug, Clone)]
pub struct HardwareProfile {
    pub backend: HardwareBackend,
    pub total_system_ram_mb: u64,
    pub total_vram_mb: u64,
    pub cpu_cores: usize,
    pub speed_estimate_tps: f64,
}

pub struct HardwareProber {
    sys: System,
}

impl HardwareProber {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        Self { sys }
    }

    pub fn probe(&self) -> HardwareProfile {
        let total_system_ram_mb = self.sys.total_memory() / 1024 / 1024;
        let cpu_cores = self.sys.physical_core_count().unwrap_or(1);

        let mut backend = HardwareBackend::CpuX86;
        let mut total_vram_mb = 0;

        // Try probing NVIDIA GPU first
        match Nvml::init() {
            Ok(nvml) => {
                match nvml.device_count() {
                    Ok(count) if count > 0 => {
                        backend = HardwareBackend::Cuda;
                        for i in 0..count {
                            if let Ok(device) = nvml.device_by_index(i) {
                                if let Ok(mem) = device.memory_info() {
                                    total_vram_mb += mem.total / 1024 / 1024;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Err(e) => {
                warn!("NVML Init failed: {:?}. Assuming no NVIDIA GPU.", e);

                // Very basic mock heuristic for Apple Silicon/Metal
                if cfg!(target_os = "macos") && std::env::consts::ARCH == "aarch64" {
                    backend = HardwareBackend::Metal;
                    // Unified memory
                    total_vram_mb = total_system_ram_mb;
                }
            }
        }

        let speed_estimate_tps = Self::estimate_speed(&backend, RunMode::FullContext);

        HardwareProfile {
            backend,
            total_system_ram_mb,
            total_vram_mb,
            cpu_cores,
            speed_estimate_tps,
        }
    }

    pub fn estimate_speed(backend: &HardwareBackend, mode: RunMode) -> f64 {
        let base_constant = match backend {
            HardwareBackend::Npu => 390.0,
            HardwareBackend::Cuda => 220.0,
            HardwareBackend::Rocm => 180.0,
            HardwareBackend::Metal => 160.0,
            HardwareBackend::Sycl => 100.0,
            HardwareBackend::CpuX86 => 70.0,
        };

        let mode_penalty = match mode {
            RunMode::FullContext => 1.0,
            RunMode::MoeExpertSwitch => 0.8,
            RunMode::CpuOffload => 0.5,
            RunMode::HalfContext => 0.5,
            RunMode::CpuOnly => 0.3,
        };

        let efficiency_factor = 0.55; // Kernel overhead and KV-cache reads

        base_constant * mode_penalty * efficiency_factor
    }
}
