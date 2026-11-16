//! The bodies, in order out from the sun: the order of the number keys.

mod moon;

use crate::body::Body;

pub fn all() -> Vec<Body> {
    vec![moon::body()]
}
