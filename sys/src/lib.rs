/* Copyright 2022 Danny McClanahan */
/* SPDX-License-Identifier: AGPL-3.0-or-later */

//! Rust wrappers for ffmpeg with bindgen!!

/* Turn all warnings into errors! */
/* #![deny(warnings)] */
/* Warn for missing docs in general, and hard require crate-level docs. */
#![warn(rustdoc::missing_crate_level_docs)]
/* Make all doctests fail if they produce any warnings. */
#![doc(test(attr(deny(warnings))))]
/* Enable all clippy lints except for many of the pedantic ones. It's a shame this needs to be
 * copied and pasted across crates, but there doesn't appear to be a way to include inner attributes
 * from a common source. */
#![deny(
  clippy::all,
  clippy::default_trait_access,
  clippy::expl_impl_clone_on_copy,
  clippy::if_not_else,
  clippy::needless_continue,
  clippy::unseparated_literal_suffix,
  clippy::used_underscore_binding
)]
/* We use inner modules in several places in this crate for ergonomics. */
#![allow(clippy::module_inception)]
/* It is often more clear to show that nothing is being moved. */
#![allow(clippy::match_ref_pats)]
/* Subjective style. */
#![allow(
  clippy::len_without_is_empty,
  clippy::redundant_field_names,
  clippy::too_many_arguments,
  clippy::single_component_path_imports
)]
/* Default isn't as big a deal as people seem to think it is. */
#![allow(clippy::new_without_default, clippy::new_ret_no_self)]
/* Arc<Mutex> can be more clear than needing to grok Orderings: */
#![allow(clippy::mutex_atomic)]
#![allow(deref_nullptr)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use cfg_if::cfg_if;

/* The #[doc = "..."] comments generated from the javadoc comments in the ffmpeg headers have code
 * samples that get parsed as doctests, so we avoid including this module when searching for
 * doctests. */
cfg_if! {
  if #[cfg(feature = "wasm")] {
    #[cfg(not(doctest))]
    pub mod bindings_wasm;
    #[cfg(not(doctest))]
    pub use crate::bindings_wasm as bindings;
  } else {
    #[cfg(not(doctest))]
    pub mod bindings_linux;
    #[cfg(not(doctest))]
    pub use crate::bindings_linux as bindings;
  }
}

#[cfg(test)]
mod tests {
  use super::bindings;

  #[test]
  fn constants() {
    assert_eq!(bindings::LIBAVUTIL_VERSION_MAJOR, 56);
  }

  #[test]
  fn linked_functions() {
    let version = unsafe { bindings::avutil_version() };
    assert!(version > bindings::LIBAVUTIL_VERSION_MAJOR);
  }
}
