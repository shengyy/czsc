//! PyO3 serialization only; all selection and numerical work is Rust-owned.
use czsc_core::analyze::CZSC;
use pyo3::{prelude::*, types::PyDict};
use pyo3_stub_gen::derive::gen_stub_pyfunction;

/// Native BI-proxy structures, role operands, stages and MACD(12,26,9).
/// di is a current-list slice, not a historical snapshot. All times are natural
/// endpoints/cutoffs; first-known times belong to the caller's prefix observer.
#[gen_stub_pyfunction]
#[pyfunction]
#[pyo3(signature=(c,di=1))]
pub fn get_structure(py: Python<'_>, c: &CZSC, di: usize) -> PyResult<Py<PyDict>> {
    let snapshot = czsc_signals::structure::get_structure(c, di);
    let json = serde_json::to_string(&snapshot)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    Ok(py
        .import("json")?
        .call_method1("loads", (json,))?
        .cast_into::<PyDict>()?
        .unbind())
}
