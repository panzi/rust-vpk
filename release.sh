#!/usr/bin/bash

set -e

VERSION=$(cargo metadata --format-version=1 --no-deps | jq -r '.packages[0].version')

cargo build --release --target x86_64-unknown-linux-gnu
cargo build --release --target x86_64-pc-windows-gnu --no-default-features

if [[ -e "tmp/rust-vpk-$VERSION" ]]; then
    rm -r "tmp/rust-vpk-$VERSION"
fi

mkdir -p "tmp/rust-vpk-$VERSION/linux-x86_64"
mkdir -p "tmp/rust-vpk-$VERSION/windows-x86_64"

cp target/x86_64-unknown-linux-gnu/release/rvpk "tmp/rust-vpk-$VERSION/linux-x86_64"
cp target/x86_64-pc-windows-gnu/release/rvpk.exe "tmp/rust-vpk-$VERSION/windows-x86_64"

cp -r README.md VPK.md Cargo.toml Cargo.lock .gitignore src release.sh "tmp/rust-vpk-$VERSION"

if [[ -e "rust-vpk-$VERSION.zip" ]]; then
    rm "rust-vpk-$VERSION.zip"
fi

pushd "tmp/rust-vpk-$VERSION"
zip -r9 "../../rust-vpk-$VERSION.zip" .
popd

rm -r "tmp/rust-vpk-$VERSION"

echo "created release ZIP: rust-vpk-$VERSION.zip"
