//! Watch error types.

use std::fmt;

#[derive(Debug, Eq, PartialEq)]
pub struct NoReceiverError;

impl fmt::Display for NoReceiverError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "所有接收器均已丢弃")
    }
}

impl std::error::Error for NoReceiverError {}

#[derive(Debug, Eq, PartialEq)]
pub struct NoSenderError;

impl fmt::Display for NoSenderError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "发送器已被丢弃")
    }
}

impl std::error::Error for NoSenderError {}
