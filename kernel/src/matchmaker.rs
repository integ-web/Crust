use crate::prober::{HardwareProfile, HardwareBackend};
use tracing::info;

#[derive(Debug, Clone)]
pub struct ModelSpec {
    pub name: String,
    pub quantization: String,
    pub required_vram_mb: u64,
}

pub struct ModelMatchmaker;

impl ModelMatchmaker {
    pub fn select_model(hardware: &HardwareProfile, models: &[ModelSpec]) -> Option<ModelSpec> {
        // We must ensure at least 2GB (2048 MB) of VRAM remains free
        // for the Browser and Context Graph, according to the requirements.
        let reserved_vram_mb = 2048;

        let available_for_model = if hardware.total_vram_mb > reserved_vram_mb {
            hardware.total_vram_mb - reserved_vram_mb
        } else if hardware.backend == HardwareBackend::CpuX86 || hardware.backend == HardwareBackend::Metal {
            // For CPU or unified memory environments with no discrete VRAM,
            // fallback to checking against System RAM, still reserving 2GB.
            if hardware.total_system_ram_mb > reserved_vram_mb {
                hardware.total_system_ram_mb - reserved_vram_mb
            } else {
                0
            }
        } else {
            0
        };

        if available_for_model == 0 {
            info!("Not enough RAM/VRAM to load any model while leaving 2GB free.");
            return None;
        }

        let mut best_model: Option<ModelSpec> = None;

        for model in models {
            if model.required_vram_mb <= available_for_model {
                match best_model.as_ref() {
                    Some(best) => {
                        // Greedily pick the one that uses the most possible available VRAM
                        // assuming higher VRAM usage = better quality/less quantization.
                        if model.required_vram_mb > best.required_vram_mb {
                            best_model = Some(model.clone());
                        }
                    }
                    None => {
                        best_model = Some(model.clone());
                    }
                }
            }
        }

        best_model
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prober::{RunMode, HardwareProber};

    #[test]
    fn test_speed_estimation() {
        let speed_cuda = HardwareProber::estimate_speed(&HardwareBackend::Cuda, RunMode::FullContext);
        assert_eq!(speed_cuda, 220.0 * 1.0 * 0.55);

        let speed_metal_moe = HardwareProber::estimate_speed(&HardwareBackend::Metal, RunMode::MoeExpertSwitch);
        assert_eq!(speed_metal_moe, 160.0 * 0.8 * 0.55);

        let speed_sycl_half = HardwareProber::estimate_speed(&HardwareBackend::Sycl, RunMode::HalfContext);
        assert_eq!(speed_sycl_half, 100.0 * 0.5 * 0.55);
    }

    #[test]
    fn test_model_matchmaking_vram_reservation() {
        let hardware = HardwareProfile {
            backend: HardwareBackend::Cuda,
            total_system_ram_mb: 16384,
            total_vram_mb: 8192, // 8GB VRAM
            cpu_cores: 8,
            speed_estimate_tps: 100.0,
        };

        let models = vec![
            ModelSpec { name: "ModelA".to_string(), quantization: "Q4_K_M".to_string(), required_vram_mb: 5000 },
            ModelSpec { name: "ModelB".to_string(), quantization: "Q8_0".to_string(), required_vram_mb: 7000 }, // Needs 7GB, Leaves 1GB (Fails)
            ModelSpec { name: "ModelC".to_string(), quantization: "IQ4_XS".to_string(), required_vram_mb: 3000 },
        ];

        // Should pick ModelA, which takes 5GB, leaving >2GB of the 8GB VRAM
        let selected = ModelMatchmaker::select_model(&hardware, &models).unwrap();
        assert_eq!(selected.name, "ModelA");
    }

    #[test]
    fn test_model_matchmaking_insufficient_vram() {
        let hardware = HardwareProfile {
            backend: HardwareBackend::Cuda,
            total_system_ram_mb: 8192,
            total_vram_mb: 4096, // 4GB VRAM
            cpu_cores: 4,
            speed_estimate_tps: 50.0,
        };

        let models = vec![
            ModelSpec { name: "ModelA".to_string(), quantization: "Q4_K_M".to_string(), required_vram_mb: 3000 }, // Needs 3GB, leaves 1GB (Fails)
        ];

        let selected = ModelMatchmaker::select_model(&hardware, &models);
        assert!(selected.is_none());
    }
}
