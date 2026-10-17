//! NumPy-style N-D array operations, vectorized evaluations, and coordinate grids
//! powered by `ndarray`.

use glam::Vec3;
use ndarray::{Array1, Array2};

/// Generates `n` evenly spaced values over [`start`, `end`], matching `np.linspace`.
pub fn linspace(start: f32, end: f32, n: usize) -> Array1<f32> {
    if n == 0 {
        return Array1::zeros(0);
    }
    if n == 1 {
        return Array1::from_elem(1, start);
    }
    let step = (end - start) / ((n - 1) as f32);
    Array1::from_shape_fn(n, |i| start + step * (i as f32))
}

/// Generates `n` evenly spaced values in double precision (f64).
pub fn linspace64(start: f64, end: f64, n: usize) -> Array1<f64> {
    if n == 0 {
        return Array1::zeros(0);
    }
    if n == 1 {
        return Array1::from_elem(1, start);
    }
    let step = (end - start) / ((n - 1) as f64);
    Array1::from_shape_fn(n, |i| start + step * (i as f64))
}

/// Generates values within half-open interval `[start, stop)` with given `step`, matching `np.arange`.
pub fn arange(start: f32, stop: f32, step: f32) -> Array1<f32> {
    if step.abs() < 1e-9 {
        return Array1::zeros(0);
    }
    let n = ((stop - start) / step).ceil().max(0.0) as usize;
    Array1::from_shape_fn(n, |i| start + step * (i as f32))
}

/// Evaluates a scalar function `f` over an array `x` in a vectorized pass.
pub fn eval_vectorized<F>(x: &Array1<f32>, f: F) -> Array1<f32>
where
    F: Fn(f32) -> f32,
{
    x.mapv(f)
}

/// Packs 1D `x` and `y` coordinate arrays into an `(N, 3)` point matrix (z = 0).
pub fn stack_points_2d(x: &Array1<f32>, y: &Array1<f32>) -> Array2<f32> {
    let n = x.len().min(y.len());
    let mut out = Array2::<f32>::zeros((n, 3));
    for i in 0..n {
        out[[i, 0]] = x[i];
        out[[i, 1]] = y[i];
        out[[i, 2]] = 0.0;
    }
    out
}

/// Packs 1D `x`, `y`, and `z` coordinate arrays into an `(N, 3)` point matrix.
pub fn stack_points_3d(x: &Array1<f32>, y: &Array1<f32>, z: &Array1<f32>) -> Array2<f32> {
    let n = x.len().min(y.len()).min(z.len());
    let mut out = Array2::<f32>::zeros((n, 3));
    for i in 0..n {
        out[[i, 0]] = x[i];
        out[[i, 1]] = y[i];
        out[[i, 2]] = z[i];
    }
    out
}

/// Convert an `(N, 3)` point matrix to a `Vec<glam::Vec3>`.
pub fn points_to_vec3(pts: &Array2<f32>) -> Vec<Vec3> {
    let n = pts.shape()[0];
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        v.push(Vec3::new(pts[[i, 0]], pts[[i, 1]], pts[[i, 2]]));
    }
    v
}

/// Convert a slice of `glam::Vec3` into an `(N, 3)` point matrix.
pub fn vec3_to_array2(pts: &[Vec3]) -> Array2<f32> {
    let n = pts.len();
    let mut out = Array2::<f32>::zeros((n, 3));
    for (i, p) in pts.iter().enumerate() {
        out[[i, 0]] = p.x;
        out[[i, 1]] = p.y;
        out[[i, 2]] = p.z;
    }
    out
}

/// 2D coordinate meshgrid matching `np.meshgrid(x, y)`.
/// Returns `(X, Y)` where each is a 2D array of shape `(y.len(), x.len())`.
pub fn meshgrid(x: &Array1<f32>, y: &Array1<f32>) -> (Array2<f32>, Array2<f32>) {
    let nx = x.len();
    let ny = y.len();
    let mut x_grid = Array2::<f32>::zeros((ny, nx));
    let mut y_grid = Array2::<f32>::zeros((ny, nx));

    for j in 0..ny {
        for i in 0..nx {
            x_grid[[j, i]] = x[i];
            y_grid[[j, i]] = y[j];
        }
    }
    (x_grid, y_grid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linspace() {
        let a = linspace(0.0, 10.0, 5);
        assert_eq!(a.len(), 5);
        assert!((a[0] - 0.0).abs() < 1e-6);
        assert!((a[1] - 2.5).abs() < 1e-6);
        assert!((a[4] - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_stack_and_conversion() {
        let x = linspace(0.0, 1.0, 3);
        let y = linspace(2.0, 3.0, 3);
        let pts = stack_points_2d(&x, &y);
        assert_eq!(pts.shape(), &[3, 3]);
        let vec = points_to_vec3(&pts);
        assert_eq!(vec.len(), 3);
        assert_eq!(vec[0], Vec3::new(0.0, 2.0, 0.0));
        assert_eq!(vec[2], Vec3::new(1.0, 3.0, 0.0));
    }
}
