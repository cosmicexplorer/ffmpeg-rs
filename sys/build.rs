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
use cfg_if::cfg_if;
use spack::{
  self,
  commands::{compiler_find::*, find::*, install::*, load::*, *},
  utils::{self, prefix},
  SpackInvocation,
};

use std::{io, path::PathBuf};

cfg_if! {
  if #[cfg(feature = "wasm")] {
    async fn ensure_ffmpeg_prefix(spack: SpackInvocation) -> Result<prefix::Prefix, spack::Error> {
      ensure_ffmpeg_prefix_wasm(spack).await
    }
  } else {
    async fn ensure_ffmpeg_prefix(spack: SpackInvocation) -> Result<prefix::Prefix, spack::Error> {
      ensure_ffmpeg_prefix_linux(spack).await
    }
  }
}

#[allow(dead_code)]
async fn ensure_ffmpeg_prefix_linux(
  spack: SpackInvocation,
) -> Result<prefix::Prefix, spack::Error> {
  let ffmpeg_prefix = utils::ensure_prefix(spack, "ffmpeg@4.4.1~alsa%gcc".into()).await?;
  Ok(ffmpeg_prefix)
}

#[allow(dead_code)]
async fn ensure_ffmpeg_prefix_wasm(spack: SpackInvocation) -> Result<prefix::Prefix, spack::Error> {
  let llvm_found_spec = utils::wasm::ensure_wasm_ready_llvm(spack.clone()).await?;

  /* FIXME: we can't use .install_find() here because `spack find` doesn't work with cli specs that
   * provide llvm's hash as a dependency of emscripten! So we instead just run find later, and pick
   * the first match with [0]. */
  let install = Install {
    spack: spack.clone(),
    spec: CLISpec(format!("emscripten@3: ^ {}", llvm_found_spec.hashed_spec())),
    verbosity: Default::default(),
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

  /* Run `spack compiler find` so it gets registered in ~/.spack/linux/compilers.yaml. */
  let compiler_find = CompilerFind {
    spack: spack.clone(),
    paths: vec![emscripten_prefix.path.clone()],
  };
  compiler_find
    .clone()
    .compiler_find()
    .await
    .map_err(|e| CommandError::CompilerFind(compiler_find, e))?;

  /* Now run our custom script to get the output of the compiler as parsed JSON. This doesn't
   * modify the global config the way CompilerFind does. */
  let find_compiler_specs = FindCompilerSpecs {
    spack: spack.clone(),
    paths: vec![emscripten_prefix.path],
  };
  let mut found_compilers = find_compiler_specs
    .clone()
    .find_compiler_specs()
    .await
    .map_err(|e| CommandError::FindCompilerSpecs(find_compiler_specs, e))?;
  assert_eq!(1, found_compilers.len());
  let emcc_found_compiler = found_compilers.pop().unwrap();
  assert!(emcc_found_compiler
    .clone()
    .into_compiler_spec_string()
    .starts_with("emscripten"));

  let load = Load {
    spack: spack.clone(),
    specs: vec![emscripten_found_spec.hashed_spec()],
  };
  let emscripten_env = load.clone().load().await.unwrap();

  let ffmpeg_for_wasm = CLISpec::new(format!(
    "ffmpeg@4.4.1+web-only%{}",
    emcc_found_compiler.into_compiler_spec_string()
  ));
  let install = Install {
    spack: spack.clone(),
    spec: ffmpeg_for_wasm.clone(),
    verbosity: Default::default(),
  };
  let () = install
    .clone()
    .install_with_env(emscripten_env)
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

async fn link_libraries(ffmpeg_prefix: prefix::Prefix) -> Result<(), prefix::PrefixTraversalError> {
  let mut needed_libraries: Vec<prefix::LibraryName> = Vec::new();
  #[cfg(feature = "libavcodec")]
  needed_libraries.push(prefix::LibraryName("avcodec".to_string()));
  #[cfg(feature = "libavdevice")]
  needed_libraries.push(prefix::LibraryName("avdevice".to_string()));
  #[cfg(feature = "libavfilter")]
  needed_libraries.push(prefix::LibraryName("avfilter".to_string()));
  #[cfg(feature = "libavformat")]
  needed_libraries.push(prefix::LibraryName("avformat".to_string()));
  #[cfg(feature = "libavutil")]
  needed_libraries.push(prefix::LibraryName("avutil".to_string()));
  #[cfg(feature = "libpostproc")]
  needed_libraries.push(prefix::LibraryName("postproc".to_string()));
  #[cfg(feature = "libswresample")]
  needed_libraries.push(prefix::LibraryName("swresample".to_string()));
  #[cfg(feature = "libswscale")]
  needed_libraries.push(prefix::LibraryName("swscale".to_string()));

  let query = prefix::LibsQuery {
    needed_libraries,
    kind: prefix::LibraryType::Dynamic,
  };
  let libs = query.find_libs(&ffmpeg_prefix).await?;

  libs.link_libraries();

  Ok(())
}

#[allow(dead_code)]
fn generate_bindings(
  ffmpeg_prefix: PathBuf,
  header_path: PathBuf,
  output_path: PathBuf,
) -> Result<(), io::Error> {
  let ffmpeg_header_root = ffmpeg_prefix.join("include");

  let bindings = bindgen::Builder::default()
    .clang_arg(format!("-I{}", ffmpeg_header_root.display()))
    .header(format!("{}", header_path.display()))
    .parse_callbacks(Box::new(bindgen::CargoCallbacks))
    .allowlist_type("AV.*")
    .allowlist_type("Swr.*")
    .allowlist_type("LIBAV.*")
    .allowlist_var("Swr.*")
    .allowlist_var("LIBAV.*")
    .allowlist_var("FF_.*")
    .allowlist_var("AV_.*")
    .allowlist_function("av.*")
    .allowlist_function("swr.*");

  /* Necessary for compiling under wasm. FIXME: only works on ubuntu!!! */
  let bindings = bindings
    .clang_arg("-I/usr/include")
    .clang_arg("-I/usr/include/x86_64-linux-gnu")
    /* See https://github.com/rust-lang/rust-bindgen/issues/1941#issuecomment-748630710. */
    .clang_arg("-fvisibility=default");

  /* We always build *all* of these libraries for the ffmpeg%emscripten spec within *spack*; we use
   * features to modify *which of these libraries gets included in your rust code*.
   * src/ffmpeg.h has ifdef blocks for each of these preprocessor defines. */
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

  let bindings = bindings.generate().expect("this is a Result<_, ()>");
  bindings.write_to_file(&output_path)?;

  Ok(())
}

#[tokio::main]
async fn main() {
  let spack = SpackInvocation::summon()
    .await
    .expect("spack summoning failed");

  let ffmpeg_prefix = ensure_ffmpeg_prefix(spack)
    .await
    .expect("finding ffmpeg failed");

  link_libraries(ffmpeg_prefix.clone())
    .await
    .expect("linking libraries should work");

  /* FIXME: fails with --feature wasm --target wasm32-unknown-unknown saying libclang.so.13 is the
   * wrong format? */
  let header_path = PathBuf::from("src/ffmpeg.h");
  let bindings_path = PathBuf::from("src/bindings.rs");
  generate_bindings(ffmpeg_prefix.path.clone(), header_path, bindings_path)
    .expect("generating bindings failed");
}
