/* Copyright 2022 Danny McClanahan */
/* SPDX-License-Identifier: AGPL-3.0-or-later */

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

use bindgen;
use spack::{
  commands::{compiler_find::*, find::*, install::*, load::*, *},
  invocation::spack::Invocation,
};

use std::path::PathBuf;

cfg_if::cfg_if! {
  if #[cfg(feature = "wasm")] {
    async fn ensure_ffmpeg_prefix(spack: Invocation) -> Result<PathBuf, spack::Error> {
      ensure_ffmpeg_prefix_wasm(spack).await
    }
  } else if #[cfg(feature = "linux")] {
    async fn ensure_ffmpeg_prefix(spack: Invocation) -> Result<PathBuf, spack::Error> {
      ensure_ffmpeg_prefix_linux(spack).await
    }
  } else {
    unreachable!("must enable either wasm or linux features at this time");
  }
}

#[allow(dead_code)]
async fn ensure_ffmpeg_prefix_linux(spack: Invocation) -> Result<PathBuf, spack::Error> {
  let ffmpeg_for_linux = CLISpec::new(format!("ffmpeg@4.4.1"));
  let install = Install {
    spack: spack.clone(),
    spec: ffmpeg_for_linux,
  };
  let ffmpeg_found_spec = install
    .clone()
    .install_find()
    .await
    .map_err(|e| CommandError::Install(install, e))?;
  let find_prefix = FindPrefix {
    spack: spack.clone(),
    spec: ffmpeg_found_spec.hashed_spec(),
  };
  let ffmpeg_prefix = find_prefix
    .clone()
    .find_prefix()
    .await
    .map_err(|e| CommandError::FindPrefix(find_prefix, e))?
    .unwrap();
  Ok(ffmpeg_prefix)
}

#[allow(dead_code)]
async fn ensure_ffmpeg_prefix_wasm(spack: Invocation) -> Result<PathBuf, spack::Error> {
  let llvm_for_wasm = CLISpec::new("llvm@14:+lld+clang+multiple-definitions~compiler-rt~tools-extra-clang~libcxx~gold~openmp~internal_unwind~polly targets=webassembly");
  let install = Install {
    spack: spack.clone(),
    spec: llvm_for_wasm,
  };
  let llvm_found_spec = install
    .clone()
    .install_find()
    .await
    .map_err(|e| CommandError::Install(install, e))?;

  let install = Install {
    spack: spack.clone(),
    spec: CLISpec(format!("emscripten@3: ^ {}", llvm_found_spec.hashed_spec())),
  };
  install
    .clone()
    .install()
    .await
    .map_err(|e| CommandError::Install(install, e))?;

  let find = Find {
    spack: spack.clone(),
    spec: CLISpec::new("emscripten@3:+create-standard-executables"),
  };
  let emscripten_found_spec = find
    .clone()
    .find()
    .await
    .map_err(|e| CommandError::Find(find, e))?[0]
    .clone();

  let find_prefix = FindPrefix {
    spack: spack.clone(),
    spec: emscripten_found_spec.hashed_spec(),
  };
  let emscripten_prefix = find_prefix
    .clone()
    .find_prefix()
    .await
    .map_err(|e| CommandError::FindPrefix(find_prefix, e))?
    .unwrap();

  let compiler_find = CompilerFind {
    spack: spack.clone(),
    paths: vec![emscripten_prefix],
  };
  let mut found_compilers = compiler_find
    .clone()
    .compiler_find()
    .await
    .map_err(|e| CommandError::CompilerFind(compiler_find, e))?;
  assert_eq!(1, found_compilers.len());
  let emcc_found_compiler = found_compilers.pop().unwrap();
  assert!(emcc_found_compiler
    .compiler_spec()
    .starts_with("emscripten"));

  let load = Load {
    spack: spack.clone(),
    specs: vec![emscripten_found_spec.hashed_spec()],
  };
  let python_env = load.clone().load().await.unwrap();

  let ffmpeg_for_wasm = CLISpec::new(format!(
    "ffmpeg@4.4.1~alsa~static~stripping%{}",
    emcc_found_compiler.compiler_spec()
  ));
  let install = Install {
    spack: spack.clone(),
    spec: ffmpeg_for_wasm.clone(),
  };
  let () = install
    .clone()
    .install_with_env(python_env)
    .await
    .map_err(|e| CommandError::Install(install, e))?;
  let find = Find {
    spack: spack.clone(),
    spec: ffmpeg_for_wasm,
  };
  let ffmpeg_found_specs = find
    .clone()
    .find()
    .await
    .map_err(|e| CommandError::Find(find, e))?;
  let ffmpeg_found_spec = ffmpeg_found_specs[0].clone();
  let find_prefix = FindPrefix {
    spack: spack.clone(),
    spec: ffmpeg_found_spec.hashed_spec(),
  };
  let ffmpeg_prefix = find_prefix
    .clone()
    .find_prefix()
    .await
    .map_err(|e| CommandError::FindPrefix(find_prefix, e))?
    .unwrap();

  Ok(ffmpeg_prefix)
}

#[tokio::main]
async fn main() -> Result<(), spack::Error> {
  let spack = Invocation::summon().await?;

  let ffmpeg_prefix = ensure_ffmpeg_prefix(spack).await?;

  let ffmpeg_header_root = ffmpeg_prefix.join("include");
  let bindings = {
    let bindings = bindgen::Builder::default()
      .clang_arg(format!("-I{}", ffmpeg_header_root.display()))
      .header("src/ffmpeg.h")
      .parse_callbacks(Box::new(bindgen::CargoCallbacks))
      .allowlist_type("AV.*")
      .allowlist_type("Swr.*")
      .allowlist_var("Swr.*")
      .allowlist_var("LIBAV.*")
      .allowlist_var("FF_.*")
      .allowlist_var("AV_.*")
      .allowlist_function("av.*")
      .allowlist_function("swr.*");

    /* We always build *all* of these libraries for the ffmpeg%emscripten spec wihin *spack*; we use
     * features to modify *which of these libraries gets included in your rust code*. */
    #[cfg(feature = "libavcodec")]
    let bindings = bindings.clang_arg("-DLIBAVCODEC");
    #[cfg(feature = "libavdevice")]
    let bindings = bindings.clang_arg("-DLIBAVDEVICE");
    #[cfg(feature = "libavfilter")]
    let bindings = bindings.clang_arg("-DLIBAVFILTER");
    #[cfg(feature = "libavformat")]
    let bindings = bindings.clang_arg("-DLIBAVFORMAT");
    #[cfg(feature = "libavutil")]
    let bindings = bindings.clang_arg("-DLIBAVUTIL");
    #[cfg(feature = "libpostproc")]
    let bindings = bindings.clang_arg("-DLIBPOSTPROC");
    #[cfg(feature = "libswresample")]
    let bindings = bindings.clang_arg("-DLIBSWRESAMPLE");
    #[cfg(feature = "libswscale")]
    let bindings = bindings.clang_arg("-DLIBSWSCALE");

    bindings.generate().expect("unable to generate bindings")
  };

  bindings
    .write_to_file("src/bindings.rs")
    .expect("couldn't write bindings");

  Ok(())
}
