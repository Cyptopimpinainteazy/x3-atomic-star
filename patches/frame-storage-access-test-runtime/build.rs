// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

fn main() {
	#[cfg(feature = "std")]
	{
		let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR must be set by cargo");
		let out_file = std::path::Path::new(&out_dir).join("wasm_binary.rs");

		// X3 local patch:
		// This crate is a benchmarking helper runtime. Its WASM artifact is not needed for
		// pallet weight generation in this repository, and the upstream wasm builder path can
		// panic in this workspace layout. Emit a stub by default and allow opt-in full build.
		if std::env::var_os("X3_ENABLE_STORAGE_ACCESS_TEST_RUNTIME_WASM").is_some() {
			substrate_wasm_builder::WasmBuilder::new()
				.with_current_project()
				.export_heap_base()
				.import_memory()
				.disable_runtime_version_section_check()
				.build();
		} else {
			std::fs::write(&out_file, "pub const WASM_BINARY: Option<&[u8]> = None;\n")
				.expect("failed to write stub wasm_binary.rs");
		}
	}
}
