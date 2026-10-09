//! Fixed-point conversions for the wire. Mirrored in `client/src/net/codec.ts`.

/// Metres to centimetres.
pub fn to_cm(metres: f32) -> i32 {
    if metres.is_finite() { (metres * 100.0).round() as i32 } else { 0 }
}

pub fn from_cm(cm: i32) -> f32 {
    cm as f32 / 100.0
}

/// Metres per second to cm/s, saturating at the i16 range.
pub fn to_cm_i16(metres: f32) -> i16 {
    if metres.is_finite() { (metres * 100.0).round().clamp(i16::MIN as f32, i16::MAX as f32) as i16 } else { 0 }
}

pub fn from_cm_i16(cm: i16) -> f32 {
    cm as f32 / 100.0
}

/// Wraps an angle to `[-π, π)`.
pub fn wrap_angle(radians: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    radians - tau * ((radians + std::f32::consts::PI) / tau).floor()
}

/// Radians to milliradians, wrapped to `[-π, π)` first so the value fits an i16.
pub fn to_mrad(radians: f32) -> i16 {
    if radians.is_finite() { (wrap_angle(radians) * 1000.0).round() as i16 } else { 0 }
}

pub fn from_mrad(mrad: i16) -> f32 {
    mrad as f32 / 1000.0
}

/// Stick axis in `[-1, 1]` to `[-127, 127]`.
pub fn to_axis(value: f32) -> i8 {
    if value.is_finite() { (value.clamp(-1.0, 1.0) * 127.0).round() as i8 } else { 0 }
}

pub fn from_axis(value: i8) -> f32 {
    (value as f32 / 127.0).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angles_wrap_into_i16() {
        assert_eq!(to_mrad(0.0), 0);
        assert_eq!(to_mrad(std::f32::consts::TAU + 1.0), 1000);
        assert_eq!(to_mrad(-std::f32::consts::PI), -3142);
        assert!(to_mrad(1.0e6).unsigned_abs() <= 3142);
    }

    #[test]
    fn non_finite_values_encode_as_zero() {
        assert_eq!(to_cm(f32::NAN), 0);
        assert_eq!(to_cm_i16(f32::INFINITY), 0);
        assert_eq!(to_axis(f32::NAN), 0);
    }
}
