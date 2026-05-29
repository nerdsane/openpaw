#!/usr/bin/env bash

# Temper WASM guests import host functions supplied by the runtime. Preserve
# those unresolved symbols as imports when building standalone .wasm artifacts.
temperpaw_configure_wasm_linker() {
    local allow_undefined="-C link-arg=--allow-undefined"

    case " ${RUSTFLAGS:-} " in
        *" ${allow_undefined} "*)
            ;;
        *)
            export RUSTFLAGS="${RUSTFLAGS:-}${RUSTFLAGS:+ }${allow_undefined}"
            ;;
    esac
}

temperpaw_configure_wasm_linker
