//! One-shot hardware peripheral initialization.
//!
//! Configures ADC channels, GPIO directions, and LEDC timers/channels
//! using raw ESP-IDF sys calls. Called once from `main()` before the
//! event loop starts.

#[cfg(target_os = "espidf")]
use esp_idf_svc::sys::*;

// ── Error type ────────────────────────────────────────────────

/// Errors during one-shot peripheral initialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HwInitError {
    AdcInitFailed(i32),
    GpioConfigFailed(i32),
    LedcInitFailed,
    IsrInstallFailed(i32),
}

impl core::fmt::Display for HwInitError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::AdcInitFailed(rc)   => write!(f, "ADC1 init failed (rc={})", rc),
            Self::GpioConfigFailed(rc) => write!(f, "GPIO config failed (rc={})", rc),
            Self::LedcInitFailed      => write!(f, "LEDC timer/channel config failed"),
            Self::IsrInstallFailed(rc) => write!(f, "GPIO ISR service install failed (rc={})", rc),
        }
    }
}



#[cfg(target_os = "espidf")]
use log::info;

use crate::pins;

#[cfg(target_os = "espidf")]
pub fn init_peripherals() -> Result<(), HwInitError> {
    // SAFETY: Called once from main() before event loop; single-threaded.
    unsafe {
        init_adc()?;
        init_gpio_inputs()?;
        init_gpio_outputs()?;
        init_ledc();
    }
    info!("hw_init: all peripherals configured");
    Ok(())
}

#[cfg(not(target_os = "espidf"))]
pub fn init_peripherals() -> Result<(), HwInitError> {
    log::info!("hw_init(sim): peripheral init skipped");
    Ok(())
}

// ── ADC (oneshot) ─────────────────────────────────────────────

#[cfg(target_os = "espidf")]
static mut ADC1_HANDLE: adc_oneshot_unit_handle_t = core::ptr::null_mut();

/// SAFETY: Must be called only from the single-threaded init path or the
/// main-loop ADC read path.  No concurrent access is possible because
/// `init_adc()` completes before the event loop starts.
#[cfg(target_os = "espidf")]
unsafe fn adc1_handle() -> adc_oneshot_unit_handle_t {
    unsafe { ADC1_HANDLE }
}

#[cfg(target_os = "espidf")]
unsafe fn init_adc() -> Result<(), HwInitError> {
    let init_cfg = adc_oneshot_unit_init_cfg_t {
        unit_id: adc_unit_t_ADC_UNIT_1,
        ulp_mode: adc_ulp_mode_t_ADC_ULP_MODE_DISABLE,
        ..Default::default()
    };
    // SAFETY: ADC1_HANDLE is only written here, once at boot.
    let ret = unsafe { adc_oneshot_new_unit(&init_cfg, &raw mut ADC1_HANDLE) };
    if ret != ESP_OK as i32 { return Err(HwInitError::AdcInitFailed(ret)); }

    let chan_cfg = adc_oneshot_chan_cfg_t {
        atten: adc_atten_t_ADC_ATTEN_DB_12,
        bitwidth: adc_bitwidth_t_ADC_BITWIDTH_12,
    };

    let ret = unsafe { adc_oneshot_config_channel(adc1_handle(), adc_channel_t_ADC_CHANNEL_4, &chan_cfg) };
    if ret != ESP_OK as i32 { return Err(HwInitError::AdcInitFailed(ret)); }

    let ret = unsafe { adc_oneshot_config_channel(adc1_handle(), adc_channel_t_ADC_CHANNEL_8, &chan_cfg) };
    if ret != ESP_OK as i32 { return Err(HwInitError::AdcInitFailed(ret)); }

    info!("hw_init: ADC1 configured (CH4=NH3, CH8=temp)");
    Ok(())
}

#[cfg(target_os = "espidf")]
pub fn adc1_read(channel: u32) -> u16 {
    let mut raw: i32 = 0;
    // SAFETY: ADC1_HANDLE is written once during init_adc() before this
    // function is called; single-threaded main-loop access guaranteed.
        // SAFETY: adc1_handle() contract — single-threaded main-loop access only.
    let ret = unsafe { adc_oneshot_read(adc1_handle(), channel, &mut raw) };
    if ret != ESP_OK as i32 {
        return 0;
    }
    raw.max(0) as u16
}

#[cfg(not(target_os = "espidf"))]
pub fn adc1_read(_channel: u32) -> u16 {
    0
}

// ── GPIO Inputs ───────────────────────────────────────────────

#[cfg(target_os = "espidf")]
unsafe fn init_gpio_inputs() -> Result<(), HwInitError> {
    let input_pins = [
        pins::WATER_LEVEL_A_GPIO,
        pins::WATER_LEVEL_B_GPIO,
        pins::FLOW_PULSE_GPIO,
        pins::UVC_INTERLOCK_GPIO,
    ];

    for &pin in &input_pins {
        let cfg = gpio_config_t {
            pin_bit_mask: 1u64 << pin,
            mode: gpio_mode_t_GPIO_MODE_INPUT,
            pull_up_en: gpio_pullup_t_GPIO_PULLUP_ENABLE,
            pull_down_en: gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
            intr_type: gpio_int_type_t_GPIO_INTR_DISABLE,
        };
        let ret = unsafe { gpio_config(&cfg) };
        if ret != ESP_OK as i32 { return Err(HwInitError::GpioConfigFailed(ret)); }
    }

    let btn_cfg = gpio_config_t {
        pin_bit_mask: 1u64 << pins::BUTTON_GPIO,
        mode: gpio_mode_t_GPIO_MODE_INPUT,
        pull_up_en: gpio_pullup_t_GPIO_PULLUP_ENABLE,
        pull_down_en: gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
        intr_type: gpio_int_type_t_GPIO_INTR_NEGEDGE,
    };
    let ret = unsafe { gpio_config(&btn_cfg) };
    if ret != ESP_OK as i32 { return Err(HwInitError::GpioConfigFailed(ret)); }

    info!("hw_init: GPIO inputs configured");
    Ok(())
}

#[cfg(target_os = "espidf")]
pub fn gpio_read(pin: i32) -> bool {
    // SAFETY: gpio_get_level is a read-only register access on an
    // already-configured input pin; safe to call from main context.
    (unsafe { gpio_get_level(pin) }) != 0
}

#[cfg(not(target_os = "espidf"))]
pub fn gpio_read(_pin: i32) -> bool {
    true
}

// ── GPIO Outputs ──────────────────────────────────────────────

#[cfg(target_os = "espidf")]
unsafe fn init_gpio_outputs() -> Result<(), HwInitError> {
    let output_pins = [
        pins::PUMP_DIR_GPIO,
        pins::UVC_ENABLE_GPIO,
    ];

    for &pin in &output_pins {
        let cfg = gpio_config_t {
            pin_bit_mask: 1u64 << pin,
            mode: gpio_mode_t_GPIO_MODE_OUTPUT,
            pull_up_en: gpio_pullup_t_GPIO_PULLUP_DISABLE,
            pull_down_en: gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
            intr_type: gpio_int_type_t_GPIO_INTR_DISABLE,
        };
        let ret = unsafe { gpio_config(&cfg) };
        if ret != ESP_OK as i32 { return Err(HwInitError::GpioConfigFailed(ret)); }
        unsafe { gpio_set_level(pin, 0) };
    }

    info!("hw_init: GPIO outputs configured");
    Ok(())
}

#[cfg(target_os = "espidf")]
pub fn gpio_write(pin: i32, high: bool) {
    // SAFETY: gpio_set_level writes to an already-configured output pin;
    // pin was validated during init_gpio_outputs(). Main-loop only.
    unsafe { gpio_set_level(pin, if high { 1 } else { 0 }); }
}

#[cfg(not(target_os = "espidf"))]
pub fn gpio_write(_pin: i32, _high: bool) {}

// ── LEDC PWM ─────────────────────────────────────────────────

#[cfg(target_os = "espidf")]
unsafe fn init_ledc() {
    // Timer 0: Pump motor (25 kHz, 8-bit)
    // SAFETY: Called from single main-task context via init_peripherals().
    let timer0 = ledc_timer_config_t {
        speed_mode: ledc_mode_t_LEDC_LOW_SPEED_MODE,
        timer_num: ledc_timer_t_LEDC_TIMER_0,
        duty_resolution: ledc_timer_bit_t_LEDC_TIMER_8_BIT,
        freq_hz: pins::PUMP_PWM_FREQ_HZ,
        clk_cfg: soc_periph_ledc_clk_src_legacy_t_LEDC_AUTO_CLK,
        ..Default::default()
    };
    unsafe { ledc_timer_config(&timer0); }

    // Timer 1: UVC + LED (1 kHz, 8-bit)
    let timer1 = ledc_timer_config_t {
        speed_mode: ledc_mode_t_LEDC_LOW_SPEED_MODE,
        timer_num: ledc_timer_t_LEDC_TIMER_1,
        duty_resolution: ledc_timer_bit_t_LEDC_TIMER_8_BIT,
        freq_hz: pins::UVC_PWM_FREQ_HZ,
        clk_cfg: soc_periph_ledc_clk_src_legacy_t_LEDC_AUTO_CLK,
        ..Default::default()
    };
    unsafe { ledc_timer_config(&timer1); }

    // Channel 0: Pump PWM
    unsafe { ledc_channel_config(&ledc_channel_config_t {
        speed_mode: ledc_mode_t_LEDC_LOW_SPEED_MODE,
        channel: ledc_channel_t_LEDC_CHANNEL_0,
        timer_sel: ledc_timer_t_LEDC_TIMER_0,
        gpio_num: pins::PUMP_PWM_GPIO,
        duty: 0,
        hpoint: 0,
        ..Default::default()
    }); }

    // Channel 1: UVC PWM
    unsafe { ledc_channel_config(&ledc_channel_config_t {
        speed_mode: ledc_mode_t_LEDC_LOW_SPEED_MODE,
        channel: ledc_channel_t_LEDC_CHANNEL_1,
        timer_sel: ledc_timer_t_LEDC_TIMER_1,
        gpio_num: pins::UVC_PWM_GPIO,
        duty: 0,
        hpoint: 0,
        ..Default::default()
    }); }

    // Channels 2-4: RGB LED
    let led_gpios = [pins::LED_R_GPIO, pins::LED_G_GPIO, pins::LED_B_GPIO];
    for (i, &gpio) in led_gpios.iter().enumerate() {
        unsafe { ledc_channel_config(&ledc_channel_config_t {
            speed_mode: ledc_mode_t_LEDC_LOW_SPEED_MODE,
            channel: (ledc_channel_t_LEDC_CHANNEL_2 + i as u32),
            timer_sel: ledc_timer_t_LEDC_TIMER_1,
            gpio_num: gpio,
            duty: 0,
            hpoint: 0,
            ..Default::default()
        }); }
    }

    info!("hw_init: LEDC configured (pump=CH0, uvc=CH1, led=CH2-4)");
}

pub const LEDC_CH_PUMP: u32 = 0;
pub const LEDC_CH_UVC: u32 = 1;
pub const LEDC_CH_LED_R: u32 = 2;
pub const LEDC_CH_LED_G: u32 = 3;
pub const LEDC_CH_LED_B: u32 = 4;

#[cfg(target_os = "espidf")]
pub fn ledc_set(channel: u32, duty: u8) {
    // SAFETY: LEDC channels were configured in init_ledc(); duty register
    // writes are race-free since only main loop calls this function.
    unsafe {
        esp_idf_svc::sys::ledc_set_duty(
            ledc_mode_t_LEDC_LOW_SPEED_MODE,
            channel,
            duty as u32,
        );
        esp_idf_svc::sys::ledc_update_duty(
            ledc_mode_t_LEDC_LOW_SPEED_MODE,
            channel,
        );
    }
}

#[cfg(not(target_os = "espidf"))]
pub fn ledc_set(_channel: u32, _duty: u8) {}

pub const ADC1_CH_NH3: u32 = 4;
pub const ADC1_CH_TEMP: u32 = 8;

// ── GPIO ISR Service ──────────────────────────────────────────

#[cfg(target_os = "espidf")]
use crate::events::{push_event, Event};
#[cfg(target_os = "espidf")]
use crate::sensors::flow::flow_isr_handler;
#[cfg(target_os = "espidf")]
use crate::drivers::button::button_isr_handler;

#[cfg(target_os = "espidf")]
unsafe extern "C" fn flow_gpio_isr(_arg: *mut core::ffi::c_void) {
    flow_isr_handler();
}

#[cfg(target_os = "espidf")]
unsafe extern "C" fn button_gpio_isr(_arg: *mut core::ffi::c_void) {
    // SAFETY: esp_timer_get_time is a RTC counter read; safe in ISR context.
    let now_ms = (unsafe { esp_idf_svc::sys::esp_timer_get_time() } / 1_000) as u32;
    button_isr_handler(now_ms);
}

#[cfg(target_os = "espidf")]
unsafe extern "C" fn interlock_gpio_isr(_arg: *mut core::ffi::c_void) {
    // LOW = closed (magnet present = safe), HIGH = open.
    // SAFETY: gpio_get_level is a register read; safe in ISR context.
    let closed = unsafe { gpio_get_level(pins::UVC_INTERLOCK_GPIO) } == 0;
    crate::sensors::set_interlock_from_isr(closed);
    push_event(Event::InterlockChanged);
}

#[cfg(target_os = "espidf")]
unsafe extern "C" fn water_level_a_isr(_arg: *mut core::ffi::c_void) {
    push_event(Event::WaterLevelChanged);
}

#[cfg(target_os = "espidf")]
unsafe extern "C" fn water_level_b_isr(_arg: *mut core::ffi::c_void) {
    push_event(Event::WaterLevelChanged);
}

/// Install per-pin GPIO ISR service and register interrupt handlers.
/// Call after init_peripherals() and before the event loop.
#[cfg(target_os = "espidf")]
pub fn init_isr_service() -> Result<(), HwInitError> {
    // SAFETY: gpio_install_isr_service is idempotent; ESP_ERR_INVALID_STATE
    // means it was already installed (acceptable). ISR handlers registered
    // below are static functions that only push to the lock-free event queue.
    unsafe {
        let ret = gpio_install_isr_service(0);
        if ret != ESP_OK && ret != ESP_ERR_INVALID_STATE {
            return Err(HwInitError::IsrInstallFailed(ret));
        }

        // Flow sensor: rising edge
        gpio_set_intr_type(pins::FLOW_PULSE_GPIO, gpio_int_type_t_GPIO_INTR_POSEDGE);
        gpio_isr_handler_add(pins::FLOW_PULSE_GPIO, Some(flow_gpio_isr), core::ptr::null_mut());
        gpio_intr_enable(pins::FLOW_PULSE_GPIO);

        // UVC interlock: any edge (lid open or close)
        gpio_set_intr_type(pins::UVC_INTERLOCK_GPIO, gpio_int_type_t_GPIO_INTR_ANYEDGE);
        gpio_isr_handler_add(pins::UVC_INTERLOCK_GPIO, Some(interlock_gpio_isr), core::ptr::null_mut());
        gpio_intr_enable(pins::UVC_INTERLOCK_GPIO);

        // Seed the interlock atomic with the current GPIO level so the
        // safety supervisor has a valid reading before the first edge fires.
        {
            let closed = gpio_get_level(pins::UVC_INTERLOCK_GPIO) == 0;
            crate::sensors::set_interlock_from_isr(closed);
        }

        // Water level A: falling edge (tank going empty)
        gpio_set_intr_type(pins::WATER_LEVEL_A_GPIO, gpio_int_type_t_GPIO_INTR_NEGEDGE);
        gpio_isr_handler_add(pins::WATER_LEVEL_A_GPIO, Some(water_level_a_isr), core::ptr::null_mut());
        gpio_intr_enable(pins::WATER_LEVEL_A_GPIO);

        // Water level B: falling edge
        gpio_set_intr_type(pins::WATER_LEVEL_B_GPIO, gpio_int_type_t_GPIO_INTR_NEGEDGE);
        gpio_isr_handler_add(pins::WATER_LEVEL_B_GPIO, Some(water_level_b_isr), core::ptr::null_mut());
        gpio_intr_enable(pins::WATER_LEVEL_B_GPIO);

        // Button: falling edge (active-low with pull-up already configured)
        gpio_set_intr_type(pins::BUTTON_GPIO, gpio_int_type_t_GPIO_INTR_NEGEDGE);
        gpio_isr_handler_add(pins::BUTTON_GPIO, Some(button_gpio_isr), core::ptr::null_mut());
        gpio_intr_enable(pins::BUTTON_GPIO);

        info!("hw_init: ISR service installed (flow, interlock, water_level×2, button)");
    }
    Ok(())
}

#[cfg(not(target_os = "espidf"))]
pub fn init_isr_service() -> Result<(), HwInitError> {
    log::info!("hw_init(sim): ISR service skipped");
    Ok(())
}
