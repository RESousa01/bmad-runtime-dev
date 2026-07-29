//! The governed model port (ENG-08).

/// Placeholder wiring; the port lands with ENG-08.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmadModelPortError {
    /// The response failed its closed output schema.
    SchemaRejected,
}
