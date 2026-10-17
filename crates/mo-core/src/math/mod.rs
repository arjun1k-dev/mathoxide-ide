//! MathOxide Numerical & Scientific Foundation:
//! The Rust high-performance equivalent of NumPy, SciPy, and nalgebra.
//!
//! - `array`: NumPy-style N-D arrays, linspace, arange, meshgrid, vectorized evaluations (`ndarray`)
//! - `bezier`: Vectorized Bézier handle calculation using ndarray slicing (`ndarray`)
//! - `transforms`: Linear algebra, matrix deformations, 2D/3D rotations, and vector fields (`nalgebra`)
//! - `complex`: Complex numbers, conformal mappings, and Möbius transformations (`num-complex`)
//! - `fft`: Fast Fourier Transforms, harmonic spectrums, and Fourier epicycles (`rustfft`)
//! - `stats`: Probability distributions (Normal, Exp, Gamma) and special functions (erf, gamma) (`statrs`)

pub mod array;
pub mod bezier;
pub mod complex;
pub mod fft;
pub mod stats;
pub mod transforms;

pub use array::{arange, eval_vectorized, linspace, linspace64, meshgrid, points_to_vec3, stack_points_2d, stack_points_3d, vec3_to_array2};
pub use bezier::{approx_smooth_quadratic_bezier_handles_ndarray, smooth_curve_from_ndarray};
pub use complex::ComplexGrid;
pub use fft::{evaluate_fourier_series, fft_1d, fourier_series_decomposition, FourierCoefficient};
pub use stats::{error_function, exponential_pdf, gamma_function, gamma_pdf, normal_cdf, normal_pdf};
pub use transforms::{apply_matrix_2d, apply_matrix_3d, rotate_2d, vector_field_2d};
