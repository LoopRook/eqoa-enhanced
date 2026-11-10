//! Circular minimap geometry retained alongside upstream rectangle placement.
pub(crate) fn circular_rim(mid: [f32; 2], q: [f32; 2], r: [f32; 4]) -> [f32; 2] {
    let d = [q[0] - mid[0], q[1] - mid[1]];
    let half = [(r[2] - r[0]) / 2.0, (r[3] - r[1]) / 2.0];
    let k = d[0].hypot(d[1]) / half[0].min(half[1]).max(1e-6);
    if k <= 1.0 {
        return q;
    }
    [mid[0] + d[0] / k, mid[1] + d[1] / k]
}
