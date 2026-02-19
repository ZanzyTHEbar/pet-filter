/**
 * ULP RISC-V program: NH3 ADC threshold monitor.
 *
 * Runs on the ESP32-S3 ULP coprocessor during deep sleep.
 * Reads ADC1_CH4 (GPIO5, MQ-137) periodically and wakes
 * the main CPU when the threshold is exceeded for consecutive
 * confirm_count samples.
 *
 * Shared variables in RTC_SLOW_MEM (accessed by main CPU via extern):
 */

#include <stdint.h>
#include "ulp_riscv.h"
#include "ulp_riscv_utils.h"
#include "ulp_riscv_adc_ulp_core.h"

/* Shared variables — names must match Rust extern declarations in power.rs */
volatile uint32_t ulp_nh3_threshold_adc = 1500;
volatile uint32_t ulp_nh3_last_reading  = 0;
volatile uint32_t ulp_nh3_above_count   = 0;
volatile uint32_t ulp_nh3_confirm_count = 3;
volatile uint32_t ulp_cycle_count       = 0;
volatile uint32_t ulp_stop_flag         = 0;

#define NH3_ADC_UNIT    ADC_UNIT_1
#define NH3_ADC_CHANNEL ADC_CHANNEL_4
#define SAMPLE_INTERVAL_US (200 * 1000)

int main(void)
{
    while (!ulp_stop_flag) {
        int32_t raw = ulp_riscv_adc_read_channel(NH3_ADC_UNIT, NH3_ADC_CHANNEL);
        if (raw < 0) {
            ulp_riscv_delay_cycles(SAMPLE_INTERVAL_US * ULP_RISCV_CYCLES_PER_US);
            continue;
        }

        ulp_nh3_last_reading = (uint32_t)raw;
        ulp_cycle_count++;

        if ((uint32_t)raw >= ulp_nh3_threshold_adc) {
            ulp_nh3_above_count++;
            if (ulp_nh3_above_count >= ulp_nh3_confirm_count) {
                ulp_riscv_wakeup_main_processor();
                ulp_riscv_halt();
            }
        } else {
            ulp_nh3_above_count = 0;
        }

        ulp_riscv_delay_cycles(SAMPLE_INTERVAL_US * ULP_RISCV_CYCLES_PER_US);
    }

    ulp_riscv_halt();
    return 0;
}
