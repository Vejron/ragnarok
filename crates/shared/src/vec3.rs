//! The arithmetic of `[f32; 3]` points and directions.

/// `point + direction * t`.
#[inline]
pub fn along(point: [f32; 3], direction: [f32; 3], t: f32) -> [f32; 3] {
    [point[0] + direction[0] * t, point[1] + direction[1] * t, point[2] + direction[2] * t]
}

/// `a - b`.
#[inline]
pub fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[inline]
pub fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
pub fn length(v: [f32; 3]) -> f32 {
    dot(v, v).sqrt()
}

#[inline]
pub fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    length(sub(a, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_and_directions() {
        assert_eq!(along([1.0, 2.0, 3.0], [0.0, 1.0, -1.0], 2.0), [1.0, 4.0, 1.0]);
        assert_eq!(sub([1.0, 2.0, 3.0], [1.0, 1.0, 1.0]), [0.0, 1.0, 2.0]);
        assert_eq!(dot([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]), 32.0);
        assert_eq!(distance([1.0, 1.0, 1.0], [4.0, 5.0, 1.0]), 5.0);
    }
}
