use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pattern {
    #[default]
    Relax,
    Wave,
    Pulse,
    Steady,
    Snake,
    Alternating,
}

impl Pattern {
    pub const ALL: [Self; 6] = [
        Self::Relax,
        Self::Wave,
        Self::Pulse,
        Self::Steady,
        Self::Snake,
        Self::Alternating,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Relax => "Релакс",
            Self::Wave => "Волна",
            Self::Pulse => "Пульс",
            Self::Steady => "Ровный",
            Self::Snake => "Змейка",
            Self::Alternating => "Лево / право",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Relax => "Медленное нарастание и мягкий спад",
            Self::Wave => "Плавные волны с короткими паузами",
            Self::Pulse => "Чередование импульсов и отдыха",
            Self::Steady => "Постоянный уровень сигнала",
            Self::Snake => "Последовательный проход по шести зонам",
            Self::Alternating => "Чередование левой и правой стороны",
        }
    }

    pub fn from_index(index: u32) -> Self {
        Self::ALL.get(index as usize).copied().unwrap_or_default()
    }

    pub fn envelope(self, t: f64, speed: f64) -> f32 {
        let t = t.max(0.0) * speed.clamp(0.5, 2.0);
        match self {
            Self::Relax => (0.15 + 0.85 * (0.5 - 0.5 * (TAU * t / 10.0).cos())) as f32,
            Self::Wave => (0.5 - 0.5 * (TAU * t / 6.0).cos()).powf(1.7) as f32,
            Self::Pulse => {
                let phase = t % 2.0;
                if phase < 0.9 {
                    (std::f64::consts::PI * phase / 0.9).sin().powi(2) as f32
                } else {
                    0.0
                }
            }
            Self::Steady => 1.0,
            Self::Snake => Self::Pulse.envelope(t, 1.0),
            Self::Alternating => (0.5 - 0.5 * (TAU * t / 6.0).cos()) as f32,
        }
    }
}

/// Wire order, recovered from uwApi.dll: seat L/R, low back L/R, mid back L/R.
pub const ZONES: [&str; 6] = [
    "Сиденье · Л",
    "Сиденье · П",
    "Низ спины · Л",
    "Низ спины · П",
    "Середина · Л",
    "Середина · П",
];

pub fn motor_levels(
    pattern: Pattern,
    t: f64,
    speed: f64,
    strength: f32,
    duration: f64,
    enabled: [bool; 6],
) -> [f32; 6] {
    let fade = session_fade(t, duration) * strength.clamp(0.0, 1.0);
    let tempo = speed.clamp(0.5, 2.0);
    std::array::from_fn(|i| {
        if !enabled[i] {
            return 0.0;
        }
        let envelope = match pattern {
            Pattern::Wave => Pattern::Wave.envelope(t + (2 - i / 2) as f64 * 2.0 / tempo, tempo),
            Pattern::Snake => {
                let order = [0, 2, 4, 5, 3, 1];
                let position = order.iter().position(|zone| *zone == i).unwrap();
                let phase = (t * tempo - position as f64 * 1.1).rem_euclid(6.6);
                if phase <= 1.65 {
                    (std::f64::consts::PI * phase / 1.65).sin().powi(2) as f32
                } else {
                    0.0
                }
            }
            Pattern::Alternating => {
                let left = 0.5 - 0.5 * (TAU * t * tempo / 6.0).cos();
                if i % 2 == 0 {
                    left as f32
                } else {
                    (1.0 - left) as f32
                }
            }
            other => other.envelope(t, tempo),
        };
        (envelope * fade).clamp(0.0, 1.0)
    })
}

pub const MAX_AMPLITUDE: f32 = 0.6;
const FADE_SECONDS: f64 = 1.2;

/// Smooth session boundaries. `t` is monotonic time since Start.
pub fn session_fade(t: f64, duration: f64) -> f32 {
    if !t.is_finite() || !duration.is_finite() || t < 0.0 || t >= duration {
        return 0.0;
    }
    let fade = (t / FADE_SECONDS)
        .min((duration - t) / FADE_SECONDS)
        .clamp(0.0, 1.0);
    (0.5 - 0.5 * (std::f64::consts::PI * fade).cos()) as f32
}

pub fn level(pattern: Pattern, t: f64, speed: f64, strength: f32, duration: f64) -> f32 {
    pattern.envelope(t, speed) * strength.clamp(0.0, 1.0) * session_fade(t, duration)
}

/// Identical mono signal is sent to all output channels. The seat itself chooses
/// how to distribute its sound-capture response among the motors.
pub fn sample(phase: f64, level: f32) -> f32 {
    ((phase.sin() + 0.18 * (2.0 * phase).sin()) / 1.18) as f32
        * level.clamp(0.0, 1.0)
        * MAX_AMPLITUDE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_at_session_boundaries_and_zero_strength() {
        for pattern in Pattern::ALL {
            assert_eq!(level(pattern, 0.0, 1.0, 1.0, 30.0), 0.0);
            assert_eq!(level(pattern, 30.0, 1.0, 1.0, 30.0), 0.0);
            assert_eq!(level(pattern, 31.0, 1.0, 1.0, 30.0), 0.0);
            assert_eq!(level(pattern, 5.0, 1.0, 0.0, 30.0), 0.0);
        }
    }

    #[test]
    fn every_pattern_is_bounded_and_pulse_has_rest() {
        for pattern in Pattern::ALL {
            for n in 0..60_000 {
                let t = n as f64 / 1000.0;
                let gain = level(pattern, t, 2.0, 1.0, 60.0);
                assert!((0.0..=1.0).contains(&gain));
                assert!(sample(t * TAU * 70.0, gain).abs() <= MAX_AMPLITUDE + 1e-6);
            }
        }
        assert_eq!(Pattern::Pulse.envelope(1.4, 1.0), 0.0);
        assert!(Pattern::Pulse.envelope(0.45, 1.0) > 0.99);
    }

    #[test]
    fn motor_masks_boundaries_and_spatial_effects() {
        for pattern in Pattern::ALL {
            for n in 0..6_000 {
                let levels = motor_levels(
                    pattern,
                    n as f64 / 100.0,
                    2.0,
                    1.0,
                    60.0,
                    [true, false, true, false, true, false],
                );
                assert!(levels.into_iter().all(|x| (0.0..=1.0).contains(&x)));
                assert_eq!(levels[1], 0.0);
                assert_eq!(levels[3], 0.0);
                assert_eq!(levels[5], 0.0);
            }
            assert_eq!(
                motor_levels(pattern, 0.0, 1.0, 1.0, 60.0, [true; 6]),
                [0.0; 6]
            );
            assert_eq!(
                motor_levels(pattern, 60.0, 1.0, 1.0, 60.0, [true; 6]),
                [0.0; 6]
            );
        }
        let left = motor_levels(Pattern::Alternating, 3.0, 1.0, 1.0, 60.0, [true; 6]);
        assert!(left[0] > 0.99 && left[1] < 0.01);
        let right = motor_levels(Pattern::Alternating, 6.0, 1.0, 1.0, 60.0, [true; 6]);
        assert!(right[1] > 0.99 && right[0] < 0.01);
    }
}
