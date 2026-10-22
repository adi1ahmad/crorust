pub mod authenticode;

pub use authenticode::{
    compute_authenticode_envelope, parse_authenticode_signature, read_p7x_from_archive,
    AuthenticodeDigests, P7X_MAGIC_HEADER,
};
