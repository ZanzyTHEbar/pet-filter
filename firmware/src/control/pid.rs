//! PID controller for pump flow rate
//!
//! Simple proportional-integral-derivative controller
//! for maintaining target water flow rate through the venturi.

/// PID controller
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
