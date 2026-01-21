// Copyright 2025 Stickerbomb Maintainers
// SPDX-License-Identifier: Apache-2.0

//! Operator internals

/// Generic Error for controller lifecycle
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Kubernetes internal error
    #[error("Kube Error: {0}")]
    KubeError(#[from] kube::Error),

    /// `serde` errors
    #[error("Serialization Error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// Kubernetes API query errors
    #[error("Parse Error: {0}")]
    ParseError(#[from] ParseGroupVersionError),

    /// Int conversion errors
    #[error("Conversion Error: {0}")]
    ConversionError(#[from] TryFromIntError),

    /// Generic string error messages
    #[error("{0}")]
    Message(String),

    /// Represents any error (currently only `rego` uses this)
    #[error("Rego Error: {0}")]
    AnyhowError(#[from] anyhow::Error),
}

impl From<String> for Error {
    fn from(msg: String) -> Self {
        Error::Message(msg)
    }
}

/// Generic result type to be used in the controller
pub type Result<T, E = Error> = std::result::Result<T, E>;

pub mod controller;
mod diagnostics;

pub mod lease;
pub mod telemetry;

use std::num::TryFromIntError;

use kube::core::gvk::ParseGroupVersionError;

pub use crate::diagnostics::*;
