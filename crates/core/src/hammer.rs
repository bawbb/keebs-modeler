//! A felt hammer: a mass thrown at the string, pushing back through a
//! nonlinear, slightly lossy spring.
//!
//! Felt force: `F = Fref · (c + α · dc/dt)`, with `c = (compression / 1 mm)^p`.
//! The exponent `p` is what makes hard strikes brighter, not just louder.

/// Compression at which `fref` is defined, m.
const REF_COMPRESSION: f64 = 1e-3;
/// Felt hysteresis at 1.0, s.
const MAX_HYSTERESIS: f64 = 4e-4;
/// Give up on the hammer this long after the strike, s.
const MAX_FLIGHT: f64 = 0.1;

#[derive(Clone, Copy, Debug, Default)]
pub struct HammerSettings {
    pub mass: f64,
    pub fref: f64,
    pub exponent: f64,
    /// 0..1
    pub hysteresis: f64,
}

#[derive(Debug, Default)]
pub struct Hammer {
    s: HammerSettings,
    /// Position, m (positive toward the string).
    u: f64,
    /// Velocity, m/s.
    v: f64,
    prev_c: f64,
    active: bool,
    in_contact: bool,
    t: f64,
    /// Total time in contact during the last strike, s.
    pub contact_time: f64,
    /// Peak force during the last strike, N.
    pub peak_force: f64,
    /// Peak felt compression during the last strike, m.
    pub peak_compression: f64,
}

impl Hammer {
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Throw the hammer at the string, starting just at contact.
    pub fn strike(&mut self, settings: HammerSettings, speed: f64, string_y: f64) {
        self.s = settings;
        self.u = string_y;
        self.v = speed;
        self.prev_c = 0.0;
        self.active = true;
        self.in_contact = false;
        self.t = 0.0;
        self.contact_time = 0.0;
        self.peak_force = 0.0;
        self.peak_compression = 0.0;
    }

    /// Advance one sample. Returns the force on the string, N.
    ///
    /// Solved implicitly so it stays stable with stiff felt and light strings:
    /// the string's displacement at the strike point next sample is
    /// `y_free + compliance · F`, where `y_free` is where it would be with no
    /// force. The hammer moves by semi-implicit Euler. We find the one `F`
    /// consistent with the felt law.
    #[inline]
    pub fn tick(&mut self, y_free: f64, compliance: f64, dt: f64) -> f64 {
        if !self.active {
            return 0.0;
        }
        let s = self.s;
        // Compression next sample, as a function of the force we apply now.
        let d0 = self.u + dt * self.v - y_free;
        let c_per_f = dt * dt / s.mass + compliance;
        let alpha = s.hysteresis * MAX_HYSTERESIS / dt;
        let felt = |d: f64| if d > 0.0 { (d / REF_COMPRESSION).powf(s.exponent) } else { 0.0 };
        let law = |f: f64| (s.fref * ((1.0 + alpha) * felt(d0 - c_per_f * f) - alpha * self.prev_c)).max(0.0);

        let force = if d0 > 0.0 {
            // h(F) = F - law(F) is increasing; root lies in [0, d0 / c_per_f].
            let (mut lo, mut hi) = (0.0, d0 / c_per_f);
            let mut f = law(0.0).min(hi);
            for _ in 0..40 {
                let h = f - law(f);
                if h.abs() <= 1e-9 * (1.0 + f) {
                    break;
                }
                if h > 0.0 { hi = f } else { lo = f }
                let d = d0 - c_per_f * f;
                let slope = if d > 0.0 && law(f) > 0.0 {
                    1.0 + s.fref * (1.0 + alpha) * s.exponent * felt(d) / d * c_per_f
                } else {
                    1.0
                };
                let next = f - h / slope;
                f = if next > lo && next < hi { next } else { 0.5 * (lo + hi) };
            }
            f
        } else {
            0.0
        };

        let compression = d0 - c_per_f * force;
        if compression > 0.0 {
            self.in_contact = true;
            self.contact_time += dt;
            self.peak_compression = self.peak_compression.max(compression);
        }
        self.prev_c = felt(compression);
        self.peak_force = self.peak_force.max(force);

        self.v -= force / s.mass * dt;
        self.u += self.v * dt;
        self.t += dt;

        // Once it has hit and is moving away, the action catches it: no
        // lingering re-contacts that would choke the string.
        let rebounded = self.in_contact && compression <= 0.0 && self.v < 0.0;
        if rebounded || self.t > MAX_FLIGHT {
            self.active = false;
        }
        force
    }
}
