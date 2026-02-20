//! PID controller for pump flow rate
//!
//! Simple proportional-integral-derivative controller
//! for maintaining target water flow rate through the venturi.

/// PID controller for venturi pump flow-rate regulation.
pub struct PidController {
    kp: f32,
    ki: f32,
    kd: f32,
    setpoint: f32,
    integral: f32,
    prev_error: f32,
    output_min: f32,
    output_max: f32,
}

impl PidController {
    pub fn new(kp: f32, ki: f32, kd: f32, setpoint: f32) -> Self {
        Self {
            kp,
            ki,
            kd,
            setpoint,
            integral: 0.0,
            prev_error: 0.0,
            output_min: 0.0,
            output_max: 100.0,
        }
    }

    /// Set output limits
    pub fn set_limits(&mut self, min: f32, max: f32) {
        self.output_min = min;
        self.output_max = max;
    }

    /// Update setpoint
    pub fn set_target(&mut self, setpoint: f32) {
        self.setpoint = setpoint;
    }

    /// Compute PID output given current measurement
    pub fn compute(&mut self, measurement: f32, dt: f32) -> f32 {
        let error = self.setpoint - measurement;

        // Proportional
        let p = self.kp * error;

        // Integral (with anti-windup)
        self.integral += error * dt;
        let i = self.ki * self.integral;

        // Derivative
        let derivative = if dt > 0.0 {
            (error - self.prev_error) / dt
        } else {
            0.0
        };
        let d = self.kd * derivative;

        self.prev_error = error;

        // Clamp output
        let output = (p + i + d).clamp(self.output_min, self.output_max);

        // Anti-windup: if output is saturated, stop integrating
        if output >= self.output_max || output <= self.output_min {
            self.integral -= error * dt;
        }

        output
    }

    /// Reset controller state
    pub fn reset(&mut self) {
        self.integral = 0.0;
        self.prev_error = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_error_zero_output() {
        let mut pid = PidController::new(1.0, 0.0, 0.0, 50.0);
        pid.set_limits(0.0, 100.0);
        let out = pid.compute(50.0, 1.0);
        assert!((out - 0.0).abs() < 0.001);
    }

    #[test]
    fn proportional_response() {
        let mut pid = PidController::new(2.0, 0.0, 0.0, 100.0);
        pid.set_limits(0.0, 200.0);
        let out = pid.compute(90.0, 1.0);
        assert!((out - 20.0).abs() < 0.001);
    }

    #[test]
    fn integral_accumulates() {
        let mut pid = PidController::new(0.0, 1.0, 0.0, 100.0);
        pid.set_limits(0.0, 200.0);
        let o1 = pid.compute(90.0, 1.0);
        let o2 = pid.compute(90.0, 1.0);
        assert!(o2 > o1, "integral should accumulate: {o2} > {o1}");
    }

    #[test]
    fn derivative_responds_to_change() {
        let mut pid = PidController::new(0.0, 0.0, 1.0, 100.0);
        pid.set_limits(-200.0, 200.0);
        let _ = pid.compute(90.0, 1.0);
        let o2 = pid.compute(95.0, 1.0);
        assert!(
            o2 < 0.0,
            "derivative should be negative when error decreases"
        );
    }

    #[test]
    fn output_clamped_to_limits() {
        let mut pid = PidController::new(100.0, 0.0, 0.0, 1000.0);
        pid.set_limits(0.0, 100.0);
        let out = pid.compute(0.0, 1.0);
        assert!((out - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn output_clamped_to_min() {
        let mut pid = PidController::new(100.0, 0.0, 0.0, 0.0);
        pid.set_limits(0.0, 100.0);
        let out = pid.compute(1000.0, 1.0);
        assert!((out - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn reset_clears_state() {
        let mut pid = PidController::new(1.0, 1.0, 1.0, 100.0);
        pid.set_limits(0.0, 200.0);
        pid.compute(50.0, 1.0);
        pid.compute(50.0, 1.0);
        pid.reset();
        let fresh = PidController::new(1.0, 1.0, 1.0, 100.0);
        assert!((pid.integral - fresh.integral).abs() < f32::EPSILON);
        assert!((pid.prev_error - fresh.prev_error).abs() < f32::EPSILON);
    }

    #[test]
    fn set_target_changes_setpoint() {
        let mut pid = PidController::new(1.0, 0.0, 0.0, 50.0);
        pid.set_limits(0.0, 200.0);
        pid.set_target(100.0);
        let out = pid.compute(90.0, 1.0);
        assert!((out - 10.0).abs() < 0.001);
    }

    #[test]
    fn zero_dt_no_derivative_explosion() {
        let mut pid = PidController::new(0.0, 0.0, 10.0, 100.0);
        pid.set_limits(-1000.0, 1000.0);
        let out = pid.compute(50.0, 0.0);
        assert!(out.is_finite());
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn output_always_clamped(
            setpoint in 0.0f32..1000.0,
            measurement in 0.0f32..2000.0,
            dt in 0.001f32..10.0,
        ) {
            let mut pid = PidController::new(10.0, 5.0, 2.0, setpoint);
            pid.set_limits(0.0, 100.0);
            let out = pid.compute(measurement, dt);
            prop_assert!((0.0..=100.0).contains(&out),
                "output {out} out of bounds for setpoint={setpoint}, meas={measurement}");
        }

        #[test]
        fn output_is_finite(
            kp in -100.0f32..100.0,
            ki in -100.0f32..100.0,
            kd in -100.0f32..100.0,
            sp in -1000.0f32..1000.0,
            meas in -1000.0f32..1000.0,
            dt in 0.0f32..100.0,
        ) {
            let mut pid = PidController::new(kp, ki, kd, sp);
            pid.set_limits(-1e6, 1e6);
            let out = pid.compute(meas, dt);
            prop_assert!(out.is_finite(), "output is not finite: {out}");
        }
    }
}
