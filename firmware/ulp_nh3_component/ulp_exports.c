#include <stdint.h>
#include "ulp_nh3_monitor.h"

// Keep generated symbols referenced from this component so they are available
// to the firmware Rust externs at link time.
void *petfilter_ulp_symbols[] = {
    (void *)&ulp_ulp_nh3_threshold_adc,
    (void *)&ulp_ulp_nh3_last_reading,
    (void *)&ulp_ulp_nh3_above_count,
    (void *)&ulp_ulp_nh3_confirm_count,
    (void *)&ulp_ulp_cycle_count,
    (void *)&ulp_ulp_stop_flag,
};
