//! NTC thermistor temperature sensor (10 kOhm @ 25 C, B = 3950).
//!
//! Wired in a voltage-divider with a fixed 10 kOhm resistor, read via
//! the ESP32-S3 ADC. The simplified Beta (Steinhart-Hart) equation
//! converts resistance to temperature.
//!
//! ## Dual-target design
//!
//! On ESP-IDF: reads ADC1_CH8 via the oneshot API (initialised by hw_init).
//! On host/test: reads from a static AtomicU16 for injection.

use core::sync::atomic::AtomicU16;
#[cfg(not(target_os = "espidf"))]
use core::sync::atomic::Ordering;

#[cfg(target_os = "espidf")]
use crate::drivers::hw_init;
static SIM_TEMP_ADC: AtomicU16 = AtomicU16::new(2048);

#[cfg(not(target_os = "espidf"))]
pub fn sim_set_temp_adc(raw: u16) {
    SIM_TEMP_ADC.store(raw, Ordering::Relaxed);
}

const R25: f32 = 10_000.0;
const BETA: f32 = 3950.0;
const T25_K: f32 = 298.15;
const R_DIVIDER: f32 = 10_000.0;
const ADC_MAX: f32 = 4095.0;
const V_REF: f32 = 3.3;

#[derive(Debug, Clone, Copy)]
pub struct TemperatureReading {
    pub raw: u16,
    pub celsius: f32,
    pub over_temp: bool,
}

pub struct TemperatureSensor {
    threshold_c: f32,
    _adc_gpio: i32,
}

impl TemperatureSensor {
    pub fn new(adc_gpio: i32, over_temp_threshold_c: f32) -> Self {
        Self {
            threshold_c: over_temp_threshold_c,
            _adc_gpio: adc_gpio,
        }
    }

    pub fn read(&self) -> TemperatureReading {
        let raw = self.read_adc();
        let celsius = self.adc_to_celsius(raw);
        TemperatureReading {
            raw,
            celsius,
            over_temp: celsius > self.threshold_c,
        }
    }

    #[cfg(target_os = "espidf")]
    fn read_adc(&self) -> u16 {
        hw_init::adc1_read(hw_init::ADC1_CH_TEMP)
    }

    #[cfg(not(target_os = "espidf"))]
    fn read_adc(&self) -> u16 {
        SIM_TEMP_ADC.load(Ordering::Relaxed)
    }

    fn adc_to_celsius(&self, raw: u16) -> f32 {
        let voltage = (raw as f32 / ADC_MAX) * V_REF;
        if voltage <= 0.01 || voltage >= (V_REF - 0.01) {
            return -40.0;
        }
        let r_ntc = R_DIVIDER * voltage / (V_REF - voltage);
        let inv_t = (1.0 / T25_K) + (1.0 / BETA) * (r_ntc / R25).ln();
        if inv_t <= 0.0 {
            return -40.0;
        }
        (1.0 / inv_t) - 273.15
    }
}
