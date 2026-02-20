//! MQ-137 ammonia (NH3) gas sensor driver.
//!
//! Reads the analog voltage output through an ESP32-S3 ADC channel,
//! applies a two-point linear calibration, and maintains a fixed-size
//! ring buffer for a running average.
//!
//! ## Dual-target design
//!
//! On ESP-IDF: reads ADC1_CH4 via the oneshot API (initialised by hw_init).
//! On host/test: reads from a static `AtomicU16` for injection.

use core::sync::atomic::{AtomicU16, Ordering};

static SIM_NH3_ADC: AtomicU16 = AtomicU16::new(0);

#[cfg(not(target_os = "espidf"))]
pub fn sim_set_nh3_adc(raw: u16) {
    SIM_NH3_ADC.store(raw, Ordering::Relaxed);
}

const RING_BUF_CAP: usize = 30;

#[derive(Debug, Clone, Copy)]
pub struct Nh3Reading {
    pub raw: u16,
    pub ppm: f32,
    pub avg_ppm: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Calibration {
    pub zero_adc: u16,
    pub span_adc: u16,
    pub span_ppm: f32,
}

impl Default for Calibration {
    fn default() -> Self {
        Self {
            zero_adc: 200,
            span_adc: 2000,
            span_ppm: 50.0,
        }
    }
}

pub struct AmmoniaSensor {
    cal: Calibration,
    ring: [f32; RING_BUF_CAP],
    head: usize,
    count: usize,
    total_reads: u32,
    _adc_gpio: i32,
}

impl AmmoniaSensor {
    pub fn new(adc_gpio: i32) -> Self {
        Self {
            cal: Calibration::default(),
            ring: [0.0; RING_BUF_CAP],
            head: 0,
            count: 0,
            total_reads: 0,
            _adc_gpio: adc_gpio,
        }
    }

    pub fn set_calibration(&mut self, cal: Calibration) {
        self.cal = cal;
    }

    pub fn is_warmed_up(&self) -> bool {
        self.total_reads >= 600
    }

    pub fn read(&mut self) -> Nh3Reading {
        self.total_reads = self.total_reads.saturating_add(1);

        let raw = self.read_adc();
        let ppm = self.adc_to_ppm(raw);

        self.ring[self.head] = ppm;
        self.head = (self.head + 1) % RING_BUF_CAP;
        if self.count < RING_BUF_CAP {
            self.count += 1;
        }

        let avg_ppm = self.running_average();
        Nh3Reading { raw, ppm, avg_ppm }
    }

    #[cfg(target_os = "espidf")]
    fn read_adc(&self) -> u16 {
        hw_init::adc1_read(hw_init::ADC1_CH_NH3)
    }

    #[cfg(not(target_os = "espidf"))]
    fn read_adc(&self) -> u16 {
        SIM_NH3_ADC.load(Ordering::Relaxed)
    }

    fn adc_to_ppm(&self, raw: u16) -> f32 {
        if raw <= self.cal.zero_adc {
            return 0.0;
        }
        let range = (self.cal.span_adc - self.cal.zero_adc) as f32;
        if range <= 0.0 {
            return 0.0;
        }
        let normalised = (raw - self.cal.zero_adc) as f32 / range;
        (normalised * self.cal.span_ppm).max(0.0)
    }

    fn running_average(&self) -> f32 {
        if self.count == 0 {
            return 0.0;
        }
        let sum: f32 = self.ring[..self.count].iter().sum();
        sum / self.count as f32
    }
}
