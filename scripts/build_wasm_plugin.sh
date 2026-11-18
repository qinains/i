#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
if [ "$#" -ne 2 ]; then printf '%s\n' 'Usage: build_wasm_plugin.sh PLUGIN_CRATE OUTPUT_DIR' >&2; exit 1; fi
crate=$(cd "$1" && pwd)
mkdir -p "$2"
output=$(cd "$2" && pwd)
stage=$(mktemp -d "$output/.plugin-build.XXXXXX")
filename=''
trap 'if [ -n "$filename" ]; then rm -f "$stage/$filename"; fi; rm -f "$stage/plugin.wasm" "$stage/manifest.json" "$stage/LICENSE"; rmdir "$stage"' EXIT
target="$(pwd)/target/wasm-plugin"
RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=--max-memory=67108864" cargo build --manifest-path "$crate/Cargo.toml" --target-dir "$target" --target wasm32-unknown-unknown --release
name=$(cargo metadata --manifest-path "$crate/Cargo.toml" --no-deps --format-version 1 | node -e 'let s="";process.stdin.on("data",v=>s+=v);process.stdin.on("end",()=>{const m=JSON.parse(s);const p=m.packages.find(p=>p.manifest_path===process.argv[1]+"/Cargo.toml");if(!p)process.exit(1);process.stdout.write(p.name.replaceAll("-","_"));});' "$crate")
cp "$target/wasm32-unknown-unknown/release/$name.wasm" "$stage/plugin.wasm"
cp "$crate/manifest.json" "$stage/manifest.json"
cp "$crate/LICENSE" "$stage/LICENSE"
filename=$(node -e 'const fs=require("fs");const m=JSON.parse(fs.readFileSync(process.argv[1]));if(!/^[a-z][a-z0-9.-]{0,63}$/.test(m.id)||!/^\d+\.\d+\.\d+$/.test(m.version))process.exit(1);process.stdout.write(m.id+"-"+m.version+".hplugin.zip");' "$stage/manifest.json")
cargo run -q -p i-wasm-package --features cli --bin i-plugin-pack -- "$stage" "$stage/$filename"
mv -f "$stage/$filename" "$output/$filename"
printf '%s\n' "$output/$filename"
