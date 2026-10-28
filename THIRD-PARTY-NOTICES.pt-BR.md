# Avisos de Terceiros — Legisladoc Sync

**Legisladoc Sync** versão 0.0.1 — Copyright (c) 2026 cwbapplab

O Legisladoc Sync é distribuído sob a [Licença MIT](LICENSE.pt-BR). Ele é
fornecido **"COMO ESTÁ" (AS IS), sem garantia de qualquer tipo**, conforme o
[aviso adicional](LICENSE.pt-BR) que acompanha a licença. O mesmo se aplica a
todos os componentes de terceiros listados neste documento.

Este documento lista os softwares de terceiros incluídos no Legisladoc Sync ou
necessários para construí-lo, a licença sob a qual cada componente é
distribuído e onde o texto completo dessa licença pode ser encontrado. Ele
abrange os crates Rust registrados em `src-tauri/Cargo.lock`, os pacotes
JavaScript/TypeScript de `package-lock.json` (instalados em `node_modules`) e
o código-fonte vendorizado em `src-tauri/vendor`.

## Sumário

- [Textos das licenças](#textos-das-licenças)
- [Código-fonte vendorizado](#código-fonte-vendorizado)
- [Crates Rust (Cargo.lock)](#crates-rust-cargolock)
- [Pacotes npm (package-lock.json)](#pacotes-npm-packagelockjson)
- [Observações sobre licenças específicas](#observações-sobre-licenças-específicas)
- [Como relatar uma omissão](#como-relatar-uma-omissão)

## Textos das licenças

O texto completo e não modificado de cada licença referenciada abaixo está no
diretório [`licenses/`](licenses/). A coluna `Componentes` indica quantos dos
475 crates Rust e 36 pacotes npm declaram aquele
identificador de licença (um componente com opção de licenças é contado em
cada uma delas).

| Identificador SPDX | Texto completo | Componentes |
| --- | --- | --- |
| [MIT](licenses/MIT.txt) | `licenses/MIT.txt` | 466 |
| [Apache-2.0](licenses/Apache-2.0.txt) | `licenses/Apache-2.0.txt` | 323 |
| [Apache-2.0 WITH LLVM-exception](licenses/Apache-2.0-LLVM-exception.txt) | `licenses/Apache-2.0-LLVM-exception.txt` | 4 |
| [BSD-3-Clause](licenses/BSD-3-Clause.txt) | `licenses/BSD-3-Clause.txt` | 9 |
| [ISC](licenses/ISC.txt) | `licenses/ISC.txt` | 6 |
| [MPL-2.0](licenses/MPL-2.0.txt) | `licenses/MPL-2.0.txt` | 7 |
| [Unicode-3.0](licenses/Unicode-3.0.txt) | `licenses/Unicode-3.0.txt` | 19 |
| [Zlib](licenses/Zlib.txt) | `licenses/Zlib.txt` | 22 |
| [0BSD](licenses/0BSD.txt) | `licenses/0BSD.txt` | 2 |
| [CC0-1.0](licenses/CC0-1.0.txt) | `licenses/CC0-1.0.txt` | 1 |
| [CDLA-Permissive-2.0](licenses/CDLA-Permissive-2.0.txt) | `licenses/CDLA-Permissive-2.0.txt` | 1 |
| [Unlicense](licenses/Unlicense.txt) | `licenses/Unlicense.txt` | 11 |
| [MIT-0](licenses/MIT-0.txt) | `licenses/MIT-0.txt` | 1 |

## Código-fonte vendorizado

### arrsync 0.2.1 — MIT

- **Caminho:** `src-tauri/vendor/arrsync`
- **Repositório original:** <https://github.com/jcaesar/ftp2mfs>
- **Copyright:** Copyright (c) Julius Michaelis <gitter@liftm.de>
- **Licença:** MIT — texto completo em [`licenses/MIT.txt`](licenses/MIT.txt)

Este crate foi vendorizado em vez de consumido do crates.io porque a versão
original entra em pânico no Windows para qualquer URL; a cópia local contém uma
correção de uma linha em `src/client.rs`. O componente, portanto, está
**modificado**. A modificação e sua justificativa estão registradas nos
comentários desse arquivo e em `src-tauri/Cargo.toml`.

## Crates Rust (Cargo.lock)

475 crates resolvidos em `src-tauri/Cargo.lock` para todos os alvos
suportados (Windows, macOS, Linux, Android, iOS e WebAssembly), incluindo
dependências de compilação e específicas de plataforma. Crates que se aplicam
apenas a outros sistemas operacionais estão listados por completude: eles não
são compilados em uma build para Windows, mas seus termos são reproduzidos aqui
para que este arquivo permaneça correto para todas as plataformas para as quais
o aplicativo pode ser compilado. Os links apontam para o projeto original; as
linhas de copyright foram extraídas dos arquivos de licença distribuídos em
cada crate. Um travessão (—) indica que o arquivo de licença do crate não traz
uma linha de copyright própria, de modo que o nome e a versão do pacote acima
identificam o componente.

| Package | Version | License | Copyright notice |
| --- | --- | --- | --- |
| [`adler2`](https://github.com/oyvindln/adler2) | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | Copyright (C) Jonas Schievink <jonasschievink@gmail.com> |
| [`aead`](https://github.com/RustCrypto/traits) | 0.5.2 | MIT OR Apache-2.0 | Copyright (c) 2019 The RustCrypto Project Developers<br>Copyright (c) 2019 MobileCoin, LLC |
| [`aes`](https://github.com/RustCrypto/block-ciphers) | 0.8.4 | MIT OR Apache-2.0 | Copyright (c) 2018 Artyom Pavlov |
| [`aes-gcm`](https://github.com/RustCrypto/AEADs) | 0.10.3 | Apache-2.0 OR MIT | Copyright (c) 2019 The RustCrypto Project Developers |
| [`aho-corasick`](https://github.com/BurntSushi/aho-corasick) | 1.1.5 | Unlicense OR MIT | Copyright (c) 2015 Andrew Gallant |
| [`alloc-no-stdlib`](https://github.com/dropbox/rust-alloc-no-stdlib) | 3.0.0 | BSD-3-Clause | Copyright (c) 2016 Dropbox, Inc. |
| [`alloc-stdlib`](https://github.com/dropbox/rust-alloc-no-stdlib) | 0.3.0 | BSD-3-Clause | — |
| [`android_log-sys`](https://github.com/rust-mobile/android_log-sys-rs) | 0.3.2 | MIT OR Apache-2.0 | Copyright 2016 The android_log_sys Developers<br>Copyright (c) 2016 The android_log_sys Developers |
| [`android_logger`](https://github.com/rust-mobile/android_logger-rs) | 0.15.1 | MIT OR Apache-2.0 | Copyright 2016 The android_logger Developers<br>Copyright (c) 2016 The android_logger Developers |
| [`android_system_properties`](https://github.com/nical/android_system_properties) | 0.1.6 | MIT OR Apache-2.0 | Copyright 2016 Nicolas Silva<br>Copyright (c) 2013 Nicolas Silva |
| [`anyhow`](https://github.com/dtolnay/anyhow) | 1.0.104 | MIT OR Apache-2.0 | — |
| [`arrayvec`](https://github.com/bluss/arrayvec) | 0.7.8 | MIT OR Apache-2.0 | Copyright (c) Ulrik Sverdrup "bluss" 2015-2023 |
| [`async-stream`](https://github.com/tokio-rs/async-stream) | 0.3.6 | MIT | Copyright (c) 2019 Carl Lerche<br>Copyright (c) 2018 David Tolnay |
| [`async-stream-impl`](https://github.com/tokio-rs/async-stream) | 0.3.6 | MIT | Copyright (c) 2019 Carl Lerche<br>Copyright (c) 2018 David Tolnay |
| [`async-trait`](https://github.com/dtolnay/async-trait) | 0.1.92 | MIT OR Apache-2.0 | — |
| [`atk`](https://github.com/gtk-rs/gtk3-rs) | 0.18.2 | MIT | — |
| [`atk-sys`](https://github.com/gtk-rs/gtk3-rs) | 0.18.2 | MIT | — |
| [`atomic-waker`](https://github.com/smol-rs/atomic-waker) | 1.1.2 | Apache-2.0 OR MIT | Copyright (c) 2016 Alex Crichton<br>Copyright (c) 2017 The Tokio Authors |
| [`autocfg`](https://github.com/cuviper/autocfg) | 1.5.1 | Apache-2.0 OR MIT | Copyright (c) 2018 Josh Stone |
| [`base64`](https://github.com/marshallpierce/rust-base64) | 0.21.7 | MIT OR Apache-2.0 | Copyright (c) 2015 Alice Maz |
| [`base64`](https://github.com/marshallpierce/rust-base64) | 0.22.1 | MIT OR Apache-2.0 | Copyright (c) 2015 Alice Maz |
| [`base64`](https://github.com/marshallpierce/rust-base64) | 0.23.1 | MIT OR Apache-2.0 | Copyright (c) 2025 Alice Maz, Marshall Pierce |
| [`bit-set`](https://github.com/contain-rs/bit-set) | 0.8.0 | Apache-2.0 OR MIT | Copyright (c) 2023 The Rust Project Developers |
| [`bit-vec`](https://github.com/contain-rs/bit-vec) | 0.8.0 | Apache-2.0 OR MIT | Copyright (c) 2023 The Rust Project Developers |
| [`bitflags`](https://github.com/bitflags/bitflags) | 1.3.2 | MIT/Apache-2.0 | Copyright (c) 2014 The Rust Project Developers |
| [`bitflags`](https://github.com/bitflags/bitflags) | 2.13.2 | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers |
| [`block-buffer`](https://github.com/RustCrypto/utils) | 0.10.4 | MIT OR Apache-2.0 | Copyright (c) 2018-2019 The RustCrypto Project Developers |
| [`block2`](https://github.com/madsmtm/objc2) | 0.6.2 | MIT | — |
| [`brotli`](https://github.com/dropbox/rust-brotli) | 9.0.0 | BSD-3-Clause AND MIT | Copyright (c) 2016 Dropbox, Inc.<br>Copyright (c) 2009, 2010, 2013-2016 by the Brotli Authors. |
| [`brotli-decompressor`](https://github.com/dropbox/rust-brotli-decompressor) | 6.0.1 | BSD-3-Clause/MIT | Copyright (c) 2016 Dropbox, Inc. |
| [`bs58`](https://github.com/Nullus157/bs58-rs) | 0.5.1 | MIT/Apache-2.0 | Copyright (c) 2016 The roaring-rs developers. |
| [`bumpalo`](https://github.com/fitzgen/bumpalo) | 3.20.3 | MIT OR Apache-2.0 | Copyright (c) 2019 Nick Fitzgerald |
| [`bytemuck`](https://github.com/Lokathor/bytemuck) | 1.25.2 | Zlib OR Apache-2.0 OR MIT | Copyright (c) 2019 Daniel "Lokathor" Gee. |
| [`byteorder`](https://github.com/BurntSushi/byteorder) | 1.5.0 | Unlicense OR MIT | Copyright (c) 2015 Andrew Gallant |
| [`bytes`](https://github.com/tokio-rs/bytes) | 1.12.1 | MIT | Copyright (c) 2018 Carl Lerche |
| [`cairo-rs`](https://github.com/gtk-rs/gtk-rs-core) | 0.18.5 | MIT | — |
| [`cairo-sys-rs`](https://github.com/gtk-rs/gtk-rs-core) | 0.18.2 | MIT | — |
| [`camino`](https://github.com/camino-rs/camino) | 1.2.6 | MIT OR Apache-2.0 | — |
| [`cargo_metadata`](https://github.com/oli-obk/cargo_metadata) | 0.19.2 | MIT | — |
| [`cargo_toml`](https://gitlab.com/lib.rs/cargo_toml) | 1.0.1 | Apache-2.0 OR MIT | — |
| [`cargo-platform`](https://github.com/rust-lang/cargo) | 0.1.9 | MIT OR Apache-2.0 | — |
| [`cc`](https://github.com/rust-lang/cc-rs) | 1.6.0 | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton |
| [`cesu8`](https://github.com/emk/cesu8-rs) | 1.1.0 | Apache-2.0/MIT | Copyright (C) 2000-2010 Julian Seward. All rights<br>Copyright (c) 2003-2013 University of Illinois at<br>Copyright (c) 2009-2014 by the contributors listed in<br>Copyright (C) 2008-2010 Tachyon Technologies.<br>Copyright (c) 2008, Natacha Porté<br>Copyright (c) 2011, Vicent Martí<br>Copyright (c) 2013, Devin Torres and the Hoedown authors<br>Copyright (C) 2012-2014 Free Software Foundation, Inc.<br>Copyright (C) 2002-2014 Jason Evans<br>Copyright (C) 2007-2012 Mozilla Foundation.<br>Copyright (C) 2009-2014 Facebook, Inc.<br>copyright itself, held by the contributor. |
| [`cfb`](https://github.com/mdsteele/rust-cfb) | 0.14.0 | MIT | Copyright (c) 2017 Matthew D. Steele |
| [`cfg-expr`](https://github.com/EmbarkStudios/cfg-expr) | 0.15.8 | MIT OR Apache-2.0 | Copyright (c) 2019 Embark Studios |
| [`cfg-if`](https://github.com/rust-lang/cfg-if) | 1.0.5 | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton |
| [`chrono`](https://github.com/chronotope/chrono) | 0.4.45 | MIT OR Apache-2.0 | Copyright (c) 2014, Kang Seonghoon. |
| [`cipher`](https://github.com/RustCrypto/traits) | 0.4.4 | MIT OR Apache-2.0 | Copyright (c) 2016-2020 RustCrypto Developers |
| [`combine`](https://github.com/Marwes/combine) | 4.6.8 | MIT | Copyright (c) 2015 Markus Westerlind |
| [`cookie`](https://github.com/SergioBenitez/cookie-rs) | 0.18.2 | MIT OR Apache-2.0 | Copyright 2017 Sergio Benitez<br>Copyright 2014 Alex Chricton<br>Copyright (c) 2017 Sergio Benitez<br>Copyright (c) 2014 Alex Crichton |
| [`core_detect`](https://github.com/thomcc/core_detect) | 1.0.0 | MIT/Apache-2.0 | Copyright (c) 2017-2020 The Rust Project Developers |
| [`core-foundation`](https://github.com/servo/core-foundation-rs) | 0.10.1 | MIT OR Apache-2.0 | Copyright (c) 2012-2013 Mozilla Foundation |
| [`core-foundation-sys`](https://github.com/servo/core-foundation-rs) | 0.8.7 | MIT OR Apache-2.0 | Copyright (c) 2012-2013 Mozilla Foundation |
| [`core-graphics`](https://github.com/servo/core-foundation-rs) | 0.25.0 | MIT OR Apache-2.0 | Copyright (c) 2012-2013 Mozilla Foundation |
| [`core-graphics-types`](https://github.com/servo/core-foundation-rs) | 0.2.0 | MIT OR Apache-2.0 | Copyright (c) 2012-2013 Mozilla Foundation |
| [`cpufeatures`](https://github.com/RustCrypto/utils) | 0.2.17 | MIT OR Apache-2.0 | Copyright (c) 2020-2025 The RustCrypto Project Developers |
| [`crc32fast`](https://github.com/srijs/rust-crc32fast) | 1.5.2 | MIT OR Apache-2.0 | Copyright (c) 2018 Sam Rijs, Alex Crichton and contributors |
| [`crossbeam-channel`](https://github.com/crossbeam-rs/crossbeam) | 0.5.17 | MIT OR Apache-2.0 | Copyright (c) 2019 The Crossbeam Project Developers<br>COPYRIGHT AND/OR OTHER APPLICABLE LAW. ANY USE OF THE WORK OTHER THAN AS<br>Copyright (c) 2009 The Go Authors. All rights reserved. |
| [`crossbeam-utils`](https://github.com/crossbeam-rs/crossbeam) | 0.8.23 | MIT OR Apache-2.0 | Copyright (c) 2019 The Crossbeam Project Developers |
| [`crypto-common`](https://github.com/RustCrypto/traits) | 0.1.7 | MIT OR Apache-2.0 | Copyright (c) 2021 RustCrypto Developers |
| [`cssparser`](https://github.com/servo/rust-cssparser) | 0.37.0 | MPL-2.0 | — |
| [`cssparser-macros`](https://github.com/servo/rust-cssparser) | 0.7.1 | MPL-2.0 | — |
| [`ctor`](https://github.com/mmastrac/linktime) | 1.0.13 | Apache-2.0 OR MIT | — |
| [`ctr`](https://github.com/RustCrypto/block-modes) | 0.9.2 | MIT OR Apache-2.0 | Copyright (c) 2018-2022 RustCrypto Developers<br>Copyright (c) 2018 Artyom Pavlov |
| [`darling`](https://github.com/TedDriggs/darling) | 0.24.1 | MIT | Copyright (c) 2017 Ted Driggs |
| [`darling_core`](https://github.com/TedDriggs/darling) | 0.24.1 | MIT | Copyright (c) 2017 Ted Driggs |
| [`darling_macro`](https://github.com/TedDriggs/darling) | 0.24.1 | MIT | Copyright (c) 2017 Ted Driggs |
| [`dbus`](https://github.com/diwic/dbus-rs) | 0.9.12 | Apache-2.0/MIT | Copyright 2014-2018 David Henningsson <diwic@ubuntu.com> and other contributors<br>Copyright (c) 2014-2018 David Henningsson <diwic@ubuntu.com> and other contributors |
| [`defmt`](https://github.com/knurling-rs/defmt) | 1.1.1 | MIT OR Apache-2.0 | Copyright (c) Ferrous Systems |
| [`defmt-macros`](https://github.com/knurling-rs/defmt) | 1.1.1 | MIT OR Apache-2.0 | Copyright (c) Ferrous Systems |
| [`defmt-parser`](https://github.com/knurling-rs/defmt) | 1.0.0 | MIT OR Apache-2.0 | — |
| [`deranged`](https://github.com/jhpratt/deranged) | 0.5.8 | MIT OR Apache-2.0 | Copyright 2024 Jacob Pratt et al.<br>Copyright (c) 2024 Jacob Pratt et al. |
| [`derive_more`](https://github.com/JelteF/derive_more) | 2.1.1 | MIT | Copyright (c) 2016 Jelte Fennema |
| [`derive_more-impl`](https://github.com/JelteF/derive_more) | 2.1.1 | MIT | Copyright (c) 2016 Jelte Fennema |
| [`digest`](https://github.com/RustCrypto/traits) | 0.10.7 | MIT OR Apache-2.0 | Copyright (c) 2017 Artyom Pavlov |
| [`dirs`](https://codeberg.org/dirs/dirs-rs) | 7.0.0 | MIT OR Apache-2.0 | Copyright (c) 2018-2019 dirs-rs contributors |
| [`dirs-sys`](https://github.com/dirs-dev/dirs-sys-rs) | 0.5.0 | MIT OR Apache-2.0 | Copyright (c) 2018-2019 dirs-rs contributors |
| [`dispatch2`](https://github.com/madsmtm/objc2) | 0.3.1 | Zlib OR Apache-2.0 OR MIT | — |
| [`displaydoc`](https://github.com/yaahc/displaydoc) | 0.2.7 | MIT OR Apache-2.0 | — |
| [`dlopen2`](https://github.com/OpenByteDev/dlopen2) | 0.8.2 | MIT | — |
| [`dlopen2_derive`](https://github.com/OpenByteDev/dlopen2) | 0.4.3 | MIT | — |
| [`dom_query`](https://github.com/niklak/dom_query) | 0.28.0 | MIT | Copyright (c) 2023 Mykola Humanov |
| [`dpi`](https://github.com/rust-windowing/winit) | 0.1.2 | Apache-2.0 AND MIT | Copyright (c) 2018 Jorge Aparicio<br>copyright:<br>Copyright © 2005-2020 Rich Felker, et al.<br>Copyright © 1993,2004 Sun Microsystems or<br>Copyright © 2003-2011 David Schultz or<br>Copyright © 2003-2009 Steven G. Kargl or<br>Copyright © 2003-2009 Bruce D. Evans or<br>Copyright © 2008 Stephen L. Moshier or<br>Copyright © 2017-2018 Arm Limited |
| [`dtoa`](https://github.com/dtolnay/dtoa) | 1.0.11 | MIT OR Apache-2.0 | — |
| [`dtoa-short`](https://github.com/upsuper/dtoa-short) | 0.3.5 | MPL-2.0 | — |
| [`dunce`](https://gitlab.com/kornelski/dunce) | 1.0.5 | CC0-1.0 OR MIT-0 OR Apache-2.0 | — |
| [`dyn-clone`](https://github.com/dtolnay/dyn-clone) | 1.0.20 | MIT OR Apache-2.0 | — |
| [`email_address`](https://github.com/johnstonskj/rust-email_address) | 0.2.9 | MIT | Copyright (c) 2019 Simon Johnston |
| [`email-encoding`](https://github.com/lettre/email-encoding) | 0.4.2 | MIT OR Apache-2.0 | Copyright (c) 2021-2024 Paolo Barbolini <paolo@paolo565.org> |
| [`embed_plist`](https://github.com/nvzqz/embed-plist-rs) | 1.2.2 | MIT OR Apache-2.0 | Copyright (c) 2020 Nikolai Vazquez |
| [`embed-resource`](https://github.com/nabijaczleweli/rust-embed-resource) | 3.0.12 | MIT | Copyright (c) 2017 nabijaczleweli |
| [`encoding_rs`](https://github.com/hsivonen/encoding_rs) | 0.8.42 | (Apache-2.0 OR MIT) AND BSD-3-Clause | Copyright Mozilla Foundation<br>Copyright © WHATWG (Apple, Google, Mozilla, Microsoft). |
| [`env_filter`](https://github.com/rust-cli/env_logger) | 0.1.4 | MIT OR Apache-2.0 | Copyright (c) Individual contributors |
| [`equivalent`](https://github.com/indexmap-rs/equivalent) | 1.0.2 | Apache-2.0 OR MIT | Copyright (c) 2016--2023 |
| [`erased-serde`](https://github.com/dtolnay/erased-serde) | 0.4.10 | MIT OR Apache-2.0 | — |
| [`fallible-iterator`](https://github.com/sfackler/rust-fallible-iterator) | 0.3.0 | MIT/Apache-2.0 | Copyright (c) 2015 The rust-openssl-verify Developers |
| [`fallible-streaming-iterator`](https://github.com/sfackler/fallible-streaming-iterator) | 0.1.9 | MIT/Apache-2.0 | Copyright (c) 2016 The fallible-streaming-iterator Developers |
| [`fastrand`](https://github.com/smol-rs/fastrand) | 2.5.0 | Apache-2.0 OR MIT | — |
| [`fdeflate`](https://github.com/image-rs/fdeflate) | 0.3.7 | MIT OR Apache-2.0 | — |
| [`fern`](https://github.com/daboross/fern) | 0.7.1 | MIT | Copyright (c) 2014-2017 David Ross |
| [`field-offset`](https://github.com/Diggsey/rust-field-offset) | 0.3.6 | MIT OR Apache-2.0 | Copyright (c) 2016-2021 Diggory Blake, and other contributors. |
| [`find-msvc-tools`](https://github.com/rust-lang/cc-rs) | 0.1.14 | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton |
| [`flate2`](https://github.com/rust-lang/flate2-rs) | 1.1.10 | MIT OR Apache-2.0 | Copyright (c) 2014-2026 Alex Crichton |
| [`fnv`](https://github.com/servo/rust-fnv) | 1.0.7 | Apache-2.0 / MIT | Copyright (c) 2017 Contributors |
| [`foldhash`](https://github.com/orlp/foldhash) | 0.2.0 | Zlib | Copyright (c) 2024 Orson Peters |
| [`foreign-types`](https://github.com/sfackler/foreign-types) | 0.5.0 | MIT/Apache-2.0 | Copyright (c) 2017 The foreign-types Developers |
| [`foreign-types-macros`](https://github.com/sfackler/foreign-types) | 0.2.4 | MIT/Apache-2.0 | Copyright (c) 2017 The foreign-types Developers |
| [`foreign-types-shared`](https://github.com/sfackler/foreign-types) | 0.3.1 | MIT/Apache-2.0 | Copyright (c) 2017 The foreign-types Developers |
| [`form_urlencoded`](https://github.com/servo/rust-url) | 1.2.2 | MIT OR Apache-2.0 | Copyright (c) 2013-2016 The rust-url developers |
| [`futures-channel`](https://github.com/rust-lang/futures-rs) | 0.3.34 | MIT OR Apache-2.0 | Copyright (c) 2016 Alex Crichton<br>Copyright (c) 2017 The Tokio Authors |
| [`futures-core`](https://github.com/rust-lang/futures-rs) | 0.3.34 | MIT OR Apache-2.0 | Copyright (c) 2016 Alex Crichton<br>Copyright (c) 2017 The Tokio Authors |
| [`futures-executor`](https://github.com/rust-lang/futures-rs) | 0.3.34 | MIT OR Apache-2.0 | Copyright (c) 2016 Alex Crichton<br>Copyright (c) 2017 The Tokio Authors |
| [`futures-io`](https://github.com/rust-lang/futures-rs) | 0.3.34 | MIT OR Apache-2.0 | Copyright (c) 2016 Alex Crichton<br>Copyright (c) 2017 The Tokio Authors |
| [`futures-macro`](https://github.com/rust-lang/futures-rs) | 0.3.34 | MIT OR Apache-2.0 | Copyright (c) 2016 Alex Crichton<br>Copyright (c) 2017 The Tokio Authors |
| [`futures-sink`](https://github.com/rust-lang/futures-rs) | 0.3.34 | MIT OR Apache-2.0 | Copyright (c) 2016 Alex Crichton<br>Copyright (c) 2017 The Tokio Authors |
| [`futures-task`](https://github.com/rust-lang/futures-rs) | 0.3.34 | MIT OR Apache-2.0 | Copyright (c) 2016 Alex Crichton<br>Copyright (c) 2017 The Tokio Authors |
| [`futures-util`](https://github.com/rust-lang/futures-rs) | 0.3.34 | MIT OR Apache-2.0 | Copyright (c) 2016 Alex Crichton<br>Copyright (c) 2017 The Tokio Authors |
| [`gdk`](https://github.com/gtk-rs/gtk3-rs) | 0.18.2 | MIT | — |
| [`gdk-pixbuf`](https://github.com/gtk-rs/gtk-rs-core) | 0.18.5 | MIT | — |
| [`gdk-pixbuf-sys`](https://github.com/gtk-rs/gtk-rs-core) | 0.18.0 | MIT | — |
| [`gdk-sys`](https://github.com/gtk-rs/gtk3-rs) | 0.18.2 | MIT | — |
| [`gdkwayland-sys`](https://github.com/gtk-rs/gtk3-rs) | 0.18.2 | MIT | — |
| [`gdkx11`](https://github.com/gtk-rs/gtk3-rs) | 0.18.2 | MIT | — |
| [`gdkx11-sys`](https://github.com/gtk-rs/gtk3-rs) | 0.18.2 | MIT | — |
| [`generic-array`](https://github.com/fizyk20/generic-array) | 0.14.7 | MIT | Copyright (c) 2015 Bartłomiej Kamiński |
| [`getrandom`](https://github.com/rust-random/getrandom) | 0.2.17 | MIT OR Apache-2.0 | Copyright (c) 2018-2024 The rust-random Project Developers<br>Copyright (c) 2014 The Rust Project Developers |
| [`getrandom`](https://github.com/rust-random/getrandom) | 0.3.4 | MIT OR Apache-2.0 | Copyright (c) 2018-2025 The rust-random Project Developers<br>Copyright (c) 2014 The Rust Project Developers |
| [`getrandom`](https://github.com/rust-random/getrandom) | 0.4.3 | MIT OR Apache-2.0 | Copyright (c) 2018-2026 The rust-random Project Developers<br>Copyright (c) 2014 The Rust Project Developers |
| [`ghash`](https://github.com/RustCrypto/universal-hashes) | 0.5.1 | Apache-2.0 OR MIT | Copyright (c) 2019 RustCrypto Developers |
| [`gio`](https://github.com/gtk-rs/gtk-rs-core) | 0.18.4 | MIT | — |
| [`gio-sys`](https://github.com/gtk-rs/gtk-rs-core) | 0.18.1 | MIT | — |
| [`glib`](https://github.com/gtk-rs/gtk-rs-core) | 0.18.5 | MIT | — |
| [`glib-macros`](https://github.com/gtk-rs/gtk-rs-core) | 0.18.5 | MIT | — |
| [`glib-sys`](https://github.com/gtk-rs/gtk-rs-core) | 0.18.1 | MIT | — |
| [`glob`](https://github.com/rust-lang/glob) | 0.3.4 | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers |
| [`gobject-sys`](https://github.com/gtk-rs/gtk-rs-core) | 0.18.0 | MIT | — |
| [`gtk`](https://github.com/gtk-rs/gtk3-rs) | 0.18.2 | MIT | — |
| [`gtk-sys`](https://github.com/gtk-rs/gtk3-rs) | 0.18.2 | MIT | — |
| [`gtk3-macros`](https://github.com/gtk-rs/gtk3-rs) | 0.18.2 | MIT | — |
| [`hashbrown`](https://github.com/rust-lang/hashbrown) | 0.12.3 | MIT OR Apache-2.0 | Copyright (c) 2016 Amanieu d'Antras |
| [`hashbrown`](https://github.com/rust-lang/hashbrown) | 0.16.1 | MIT OR Apache-2.0 | Copyright (c) 2016 Amanieu d'Antras |
| [`hashbrown`](https://github.com/rust-lang/hashbrown) | 0.17.1 | MIT OR Apache-2.0 | Copyright (c) 2016 Amanieu d'Antras |
| [`hashlink`](https://github.com/djc/hashlink) | 0.12.2 | MIT OR Apache-2.0 | — |
| [`heck`](https://github.com/withoutboats/heck) | 0.4.1 | MIT OR Apache-2.0 | Copyright (c) 2015 The Rust Project Developers |
| [`heck`](https://github.com/withoutboats/heck) | 0.5.0 | MIT OR Apache-2.0 | Copyright (c) 2015 The Rust Project Developers |
| [`hex`](https://github.com/KokaKiwi/rust-hex) | 0.4.3 | MIT OR Apache-2.0 | Copyright (c) 2013-2014 The Rust Project Developers.<br>Copyright (c) 2015-2020 The rust-hex Developers |
| [`hexdump`](https://github.com/tbu-/hexdump) | 0.1.3 | MIT/Apache-2.0 | Copyright (c) 2016 Tobias Bucher |
| [`hostname`](https://github.com/djc/hostname) | 0.4.2 | MIT | Copyright (c) 2016 fengcen<br>Copyright (c) 2019 svartalf |
| [`html5ever`](https://github.com/servo/html5ever) | 0.39.0 | MIT OR Apache-2.0 | Copyright (c) 2014 The html5ever Project Developers |
| [`http`](https://github.com/hyperium/http) | 1.5.0 | MIT OR Apache-2.0 | Copyright 2017 http-rs authors<br>Copyright (c) 2017 http-rs authors |
| [`http-body`](https://github.com/hyperium/http-body) | 1.1.0 | MIT | Copyright (c) 2019-2026 Sean McArthur & Hyper Contributors |
| [`http-body-util`](https://github.com/hyperium/http-body) | 0.1.5 | MIT | Copyright (c) 2019-2026 Sean McArthur & Hyper Contributors |
| [`httparse`](https://github.com/seanmonstar/httparse) | 1.10.1 | MIT OR Apache-2.0 | Copyright (c) 2015-2025 Sean McArthur |
| [`httpdate`](https://github.com/pyfisch/httpdate) | 1.0.3 | MIT OR Apache-2.0 | Copyright (c) 2016 Pyfisch |
| [`hyper`](https://github.com/hyperium/hyper) | 1.12.0 | MIT | Copyright (c) 2014-2026 Sean McArthur |
| [`hyper-util`](https://github.com/hyperium/hyper-util) | 0.1.21 | MIT | Copyright (c) 2023-2025 Sean McArthur |
| [`iana-time-zone`](https://github.com/strawlab/iana-time-zone) | 0.1.65 | MIT OR Apache-2.0 | Copyright 2020 Andrew Straw<br>Copyright (c) 2020 Andrew D. Straw |
| [`iana-time-zone-haiku`](https://github.com/strawlab/iana-time-zone) | 0.1.2 | MIT OR Apache-2.0 | Copyright 2020 Andrew Straw<br>Copyright (c) 2020 Andrew D. Straw |
| [`ico`](https://github.com/mdsteele/rust-ico) | 0.5.0 | MIT | Copyright (c) 2018 Matthew D. Steele |
| [`icu_collections`](https://github.com/unicode-org/icu4x) | 2.3.0 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| [`icu_locale_core`](https://github.com/unicode-org/icu4x) | 2.3.0 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| [`icu_normalizer`](https://github.com/unicode-org/icu4x) | 2.3.0 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| [`icu_normalizer_data`](https://github.com/unicode-org/icu4x) | 2.3.0 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| [`icu_properties`](https://github.com/unicode-org/icu4x) | 2.3.0 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| [`icu_properties_data`](https://github.com/unicode-org/icu4x) | 2.3.0 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| [`icu_provider`](https://github.com/unicode-org/icu4x) | 2.3.1 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| [`ident_case`](https://github.com/TedDriggs/ident_case) | 1.0.1 | MIT/Apache-2.0 | — |
| [`idna`](https://github.com/servo/rust-url/) | 1.1.0 | MIT OR Apache-2.0 | Copyright (c) 2013-2025 The rust-url developers |
| [`idna_adapter`](https://github.com/hsivonen/idna_adapter) | 1.2.2 | Apache-2.0 OR MIT | Copyright (c) The rust-url developers |
| [`indexmap`](https://github.com/bluss/indexmap) | 1.9.3 | Apache-2.0 OR MIT | Copyright (c) 2016--2017 |
| [`indexmap`](https://github.com/indexmap-rs/indexmap) | 2.14.2 | Apache-2.0 OR MIT | Copyright (c) 2016--2017 |
| [`infer`](https://github.com/bojand/infer) | 0.22.0 | MIT | Copyright (c) 2019 Bojan |
| [`inout`](https://github.com/RustCrypto/utils) | 0.1.4 | MIT OR Apache-2.0 | Copyright (c) 2022 The RustCrypto Project Developers<br>Copyright (c) 2022 Artyom Pavlov |
| [`ipnet`](https://github.com/krisprice/ipnet) | 2.12.2 | MIT OR Apache-2.0 | Copyright 2017 Juniper Networks, Inc. |
| [`itoa`](https://github.com/dtolnay/itoa) | 1.0.18 | MIT OR Apache-2.0 | — |
| [`javascriptcore-rs`](https://github.com/tauri-apps/javascriptcore-rs) | 1.1.2 | MIT | Copyright (c) 2013-2021, The Gtk-rs Project Developers.<br>Copyright (c) 2021, Tauri Programme within The Commons Conservancy. |
| [`javascriptcore-rs-sys`](https://github.com/tauri-apps/javascriptcore-rs) | 1.1.1 | MIT | Copyright (c) 2013-2017, The Gtk-rs Project Developers. |
| [`jiff`](https://github.com/BurntSushi/jiff) | 0.2.38 | Unlicense OR MIT | Copyright (c) 2015 Andrew Gallant |
| [`jiff-core`](https://github.com/BurntSushi/jiff) | 0.1.1 | Unlicense OR MIT | Copyright (c) 2015 Andrew Gallant |
| [`jiff-static`](https://github.com/BurntSushi/jiff) | 0.2.38 | Unlicense OR MIT | Copyright (c) 2015 Andrew Gallant |
| [`jiff-tzdb`](https://github.com/BurntSushi/jiff) | 0.1.9 | Unlicense OR MIT | Copyright (c) 2015 Andrew Gallant |
| [`jiff-tzdb-platform`](https://github.com/BurntSushi/jiff) | 0.1.3 | Unlicense OR MIT | Copyright (c) 2015 Andrew Gallant |
| [`jni`](https://github.com/jni-rs/jni-rs) | 0.21.1 | MIT/Apache-2.0 | Copyright (c) 2016 Prevoty, Inc. and jni-rs contributors |
| [`jni-sys`](https://github.com/jni-rs/jni-sys) | 0.3.1 | MIT OR Apache-2.0 | Copyright (c) 2015 The rust-jni-sys Developers |
| [`jni-sys`](https://github.com/jni-rs/jni-sys) | 0.4.1 | MIT OR Apache-2.0 | Copyright (c) 2015 The rust-jni-sys Developers |
| [`jni-sys-macros`](https://github.com/jni-rs/jni-sys) | 0.4.1 | MIT OR Apache-2.0 | — |
| [`js-sys`](https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/js-sys) | 0.3.106 | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton |
| [`json-patch`](https://github.com/idubrov/json-patch) | 4.2.0 | MIT/Apache-2.0 | Copyright (c) 2017 Ivan Dubrov |
| [`jsonptr`](https://github.com/chanced/jsonptr) | 0.7.1 | MIT OR Apache-2.0 | Copyright 2024 Chance Dinkins<br>Copyright (c) 2022 Chance Dinkins |
| [`keyboard-types`](https://github.com/rust-windowing/keyboard-types) | 0.8.3 | MIT OR Apache-2.0 | Copyright (c) 2017 Pyfisch |
| [`lettre`](https://github.com/lettre/lettre) | 0.11.23 | MIT | Copyright (c) 2014-2024 Alexis Mousset <contact@amousset.me><br>Copyright (c) 2019-2025 Paolo Barbolini <paolo@paolo565.org><br>Copyright (c) 2018 K. <kayo@illumium.org> |
| `libappindicator` | 0.9.0 | Apache-2.0 OR MIT | Copyright (c) 2017-2021 qDot<br>Copyright (c) 2021 Tauri Apps Contributors |
| `libappindicator-sys` | 0.9.0 | Apache-2.0 OR MIT | — |
| [`libc`](https://github.com/rust-lang/libc) | 0.2.190 | MIT OR Apache-2.0 | Copyright (c) The Rust Project Developers |
| [`libdbus-sys`](https://github.com/diwic/dbus-rs) | 0.2.7 | Apache-2.0/MIT | Copyright 2014-2018 David Henningsson <diwic@ubuntu.com> and other contributors<br>Copyright (c) 2014-2018 David Henningsson <diwic@ubuntu.com> and other contributors |
| [`libloading`](https://github.com/nagisa/rust_libloading/) | 0.7.4 | ISC | Copyright © 2015, Simonas Kazlauskas |
| [`libredox`](https://gitlab.redox-os.org/redox-os/libredox) | 0.1.25 | MIT | Copyright (c) 2023 4lDO2 |
| [`libsqlite3-sys`](https://github.com/rusqlite/rusqlite) | 0.38.2 | MIT | Copyright (c) 2014 The rusqlite developers |
| [`litemap`](https://github.com/unicode-org/icu4x) | 0.8.3 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| [`lock_api`](https://github.com/Amanieu/parking_lot) | 0.4.14 | MIT OR Apache-2.0 | Copyright (c) 2016 The Rust Project Developers |
| [`log`](https://github.com/rust-lang/log) | 0.4.34 | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers |
| [`markup5ever`](https://github.com/servo/html5ever) | 0.39.0 | MIT OR Apache-2.0 | Copyright (c) 2014 The html5ever Project Developers |
| [`memchr`](https://github.com/BurntSushi/memchr) | 2.8.3 | Unlicense OR MIT | Copyright (c) 2015 Andrew Gallant |
| [`memoffset`](https://github.com/Gilnaa/memoffset) | 0.9.1 | MIT | Copyright (c) 2017 Gilad Naaman |
| [`mime`](https://github.com/hyperium/mime) | 0.3.17 | MIT OR Apache-2.0 | Copyright (c) 2014 Sean McArthur |
| [`miniz_oxide`](https://github.com/Frommi/miniz_oxide/tree/master/miniz_oxide) | 0.8.9 | MIT OR Zlib OR Apache-2.0 | Copyright 2013-2014 RAD Game Tools and Valve Software<br>Copyright 2010-2014 Rich Geldreich and Tenacious Software LLC<br>Copyright (c) 2017 Frommi<br>Copyright (c) 2017-2024 oyvindln<br>Copyright (c) 2020 Frommi |
| [`miniz_oxide`](https://github.com/Frommi/miniz_oxide/tree/master/miniz_oxide) | 0.9.1 | MIT OR Zlib OR Apache-2.0 | Copyright 2013-2014 RAD Game Tools and Valve Software<br>Copyright 2010-2014 Rich Geldreich and Tenacious Software LLC<br>Copyright (c) 2017 Frommi<br>Copyright (c) 2017-2024 oyvindln<br>Copyright (c) 2020 Frommi |
| [`mio`](https://github.com/tokio-rs/mio) | 1.2.4 | MIT | Copyright (c) 2014 Carl Lerche and other MIO contributors |
| [`muda`](https://github.com/tauri-apps/muda) | 0.20.0 | Apache-2.0 OR MIT | Copyright (c) 2022-2022 Tauri Programme within The Commons Conservancy |
| [`multiversion_no_op`](https://github.com/hsivonen/multiversion_no_op) | 1.0.0 | Apache-2.0 OR MIT | Copyright Mozilla Foundation |
| [`ndk`](https://github.com/rust-mobile/ndk) | 0.9.0 | MIT OR Apache-2.0 | — |
| [`ndk-context`](https://github.com/rust-windowing/android-ndk-rs) | 0.1.1 | MIT OR Apache-2.0 | — |
| [`ndk-sys`](https://github.com/rust-mobile/ndk) | 0.6.0+11769913 | MIT OR Apache-2.0 | — |
| [`new_debug_unreachable`](https://github.com/mbrubeck/rust-debug-unreachable) | 1.0.6 | MIT | Copyright (c) 2015 Jonathan Reem |
| [`nom`](https://github.com/rust-bakery/nom) | 8.0.0 | MIT | Copyright (c) 2014-2019 Geoffroy Couprie |
| [`num_enum`](https://github.com/illicitonion/num_enum) | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 | Copyright (c) 2018, Daniel Wagner-Hall |
| [`num_enum_derive`](https://github.com/illicitonion/num_enum) | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 | Copyright (c) 2018, Daniel Wagner-Hall |
| [`num_threads`](https://github.com/jhpratt/num_threads) | 0.1.7 | MIT OR Apache-2.0 | Copyright 2021 Jacob Pratt<br>Copyright (c) 2021 Jacob Pratt |
| [`num-conv`](https://github.com/jhpratt/num-conv) | 0.2.2 | MIT OR Apache-2.0 | Copyright (c) Jacob Pratt |
| [`num-traits`](https://github.com/rust-num/num-traits) | 0.2.19 | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers |
| [`objc2`](https://github.com/madsmtm/objc2) | 0.6.5 | MIT | — |
| [`objc2-app-kit`](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT | — |
| [`objc2-cloud-kit`](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT | — |
| [`objc2-core-data`](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT | — |
| [`objc2-core-foundation`](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT | — |
| [`objc2-core-graphics`](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT | — |
| [`objc2-core-image`](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT | — |
| [`objc2-core-location`](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT | — |
| [`objc2-core-text`](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT | — |
| [`objc2-encode`](https://github.com/madsmtm/objc2) | 4.1.0 | MIT | — |
| [`objc2-exception-helper`](https://github.com/madsmtm/objc2) | 0.1.1 | Zlib OR Apache-2.0 OR MIT | — |
| [`objc2-foundation`](https://github.com/madsmtm/objc2) | 0.3.2 | MIT | — |
| [`objc2-io-surface`](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT | — |
| [`objc2-quartz-core`](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT | — |
| [`objc2-ui-kit`](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT | — |
| [`objc2-user-notifications`](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT | — |
| [`objc2-web-kit`](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT | — |
| [`once_cell`](https://github.com/matklad/once_cell) | 1.21.4 | MIT OR Apache-2.0 | — |
| [`opaque-debug`](https://github.com/RustCrypto/utils) | 0.3.1 | MIT OR Apache-2.0 | Copyright (c) 2018-2024 The RustCrypto Project Developers |
| [`option-ext`](https://github.com/soc/option-ext) | 0.2.0 | MPL-2.0 | — |
| [`pango`](https://github.com/gtk-rs/gtk-rs-core) | 0.18.3 | MIT | — |
| [`pango-sys`](https://github.com/gtk-rs/gtk-rs-core) | 0.18.0 | MIT | — |
| [`parking_lot`](https://github.com/Amanieu/parking_lot) | 0.12.5 | MIT OR Apache-2.0 | Copyright (c) 2016 The Rust Project Developers |
| [`parking_lot_core`](https://github.com/Amanieu/parking_lot) | 0.9.12 | MIT OR Apache-2.0 | Copyright (c) 2016 The Rust Project Developers |
| [`percent-encoding`](https://github.com/servo/rust-url/) | 2.3.2 | MIT OR Apache-2.0 | Copyright (c) 2013-2025 The rust-url developers |
| [`phf`](https://github.com/rust-phf/rust-phf) | 0.13.1 | MIT | Copyright (c) 2014-2022 Steven Fackler, Yuki Okushi |
| [`phf_codegen`](https://github.com/rust-phf/rust-phf) | 0.13.1 | MIT | Copyright (c) 2014-2022 Steven Fackler, Yuki Okushi |
| [`phf_generator`](https://github.com/rust-phf/rust-phf) | 0.13.1 | MIT | Copyright (c) 2014-2022 Steven Fackler, Yuki Okushi |
| [`phf_macros`](https://github.com/rust-phf/rust-phf) | 0.13.1 | MIT | Copyright (c) 2014-2022 Steven Fackler, Yuki Okushi |
| [`phf_shared`](https://github.com/rust-phf/rust-phf) | 0.13.1 | MIT | Copyright (c) 2014-2022 Steven Fackler, Yuki Okushi |
| [`pin-project-lite`](https://github.com/taiki-e/pin-project-lite) | 0.2.17 | Apache-2.0 OR MIT | — |
| [`pkg-config`](https://github.com/rust-lang/pkg-config-rs) | 0.3.34 | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton |
| [`plist`](https://github.com/ebarnard/rust-plist/) | 1.10.1 | MIT | Copyright (c) 2015 Edward Barnard |
| [`png`](https://github.com/image-rs/image-png) | 0.17.16 | MIT OR Apache-2.0 | Copyright (c) 2015 nwin |
| [`png`](https://github.com/image-rs/image-png) | 0.18.1 | MIT OR Apache-2.0 | Copyright (c) 2015 nwin |
| [`polyval`](https://github.com/RustCrypto/universal-hashes) | 0.6.2 | Apache-2.0 OR MIT | Copyright (c) 2019-2023 RustCrypto Developers |
| [`portable-atomic`](https://github.com/taiki-e/portable-atomic) | 1.15.0 | Apache-2.0 OR MIT | — |
| [`portable-atomic-util`](https://github.com/taiki-e/portable-atomic-util) | 0.2.8 | Apache-2.0 OR MIT | — |
| [`potential_utf`](https://github.com/unicode-org/icu4x) | 0.1.6 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| [`powerfmt`](https://github.com/jhpratt/powerfmt) | 0.2.1 | MIT OR Apache-2.0 | Copyright 2023 Jacob Pratt et al.<br>Copyright (c) 2023 Jacob Pratt et al. |
| [`precomputed-hash`](https://github.com/emilio/precomputed-hash) | 0.1.1 | MIT | Copyright (c) 2017 Emilio Cobos Álvarez |
| [`proc-macro-crate`](https://github.com/bkchr/proc-macro-crate) | 1.3.1 | MIT OR Apache-2.0 | — |
| [`proc-macro-crate`](https://github.com/bkchr/proc-macro-crate) | 2.0.2 | MIT OR Apache-2.0 | — |
| [`proc-macro-crate`](https://github.com/bkchr/proc-macro-crate) | 3.5.0 | MIT OR Apache-2.0 | — |
| [`proc-macro-error`](https://gitlab.com/CreepySkeleton/proc-macro-error) | 1.0.4 | MIT OR Apache-2.0 | Copyright 2019-2020 CreepySkeleton <creepy-skeleton@yandex.ru><br>Copyright (c) 2019-2020 CreepySkeleton |
| [`proc-macro-error-attr`](https://gitlab.com/CreepySkeleton/proc-macro-error) | 1.0.4 | MIT OR Apache-2.0 | Copyright 2019-2020 CreepySkeleton <creepy-skeleton@yandex.ru><br>Copyright (c) 2019-2020 CreepySkeleton |
| [`proc-macro2`](https://github.com/dtolnay/proc-macro2) | 1.0.107 | MIT OR Apache-2.0 | — |
| [`quick-xml`](https://github.com/tafia/quick-xml) | 0.42.0 | MIT | Copyright (c) 2016 Johann Tuffe |
| [`quote`](https://github.com/dtolnay/quote) | 1.0.47 | MIT OR Apache-2.0 | — |
| [`quoted_printable`](https://github.com/staktrace/quoted-printable) | 0.5.2 | 0BSD | Copyright (C) 2019 by Kartikaya Gupta |
| [`r-efi`](https://github.com/r-efi/r-efi) | 5.3.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | — |
| [`r-efi`](https://github.com/r-efi/r-efi) | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | — |
| [`rand_core`](https://github.com/rust-random/rand) | 0.6.4 | MIT OR Apache-2.0 | copyright assignment is required to contribute to the Rand project.<br>Copyright 2018 Developers of the Rand project<br>Copyright (c) 2014 The Rust Project Developers |
| [`raw-window-handle`](https://github.com/rust-windowing/raw-window-handle) | 0.6.2 | MIT OR Apache-2.0 OR Zlib | Copyright (c) 2019 Osspial<br>Copyright (c) 2020 Osspial |
| [`redox_syscall`](https://gitlab.redox-os.org/redox-os/syscall) | 0.5.18 | MIT | Copyright (c) 2017 Redox OS Developers |
| [`redox_users`](https://gitlab.redox-os.org/redox-os/users) | 0.5.3 | MIT | Copyright (c) 2017 Jose Narvaez |
| [`ref-cast`](https://github.com/dtolnay/ref-cast) | 1.0.27 | MIT OR Apache-2.0 | — |
| [`ref-cast-impl`](https://github.com/dtolnay/ref-cast) | 1.0.27 | MIT OR Apache-2.0 | — |
| [`regex`](https://github.com/rust-lang/regex) | 1.13.1 | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers |
| [`regex-automata`](https://github.com/rust-lang/regex) | 0.4.18 | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers |
| [`regex-syntax`](https://github.com/rust-lang/regex) | 0.8.11 | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers |
| [`reqwest`](https://github.com/seanmonstar/reqwest) | 0.13.5 | MIT OR Apache-2.0 | Copyright 2016 Sean McArthur<br>Copyright (c) 2016-2026 Sean McArthur |
| [`ring`](https://github.com/briansmith/ring) | 0.17.14 | Apache-2.0 AND ISC | Copyright (c) 2009 The Go Authors. All rights reserved.<br>Copyright 2015 The Chromium Authors. All rights reserved.<br>Copyright 2015-2025 Brian Smith. |
| [`roxmltree`](https://github.com/RazrFalcon/roxmltree) | 0.21.1 | MIT OR Apache-2.0 | Copyright (c) 2018 Yevhenii Reizner |
| `rsqlite-vfs` | 0.1.1 | MIT | — |
| [`rusqlite`](https://github.com/rusqlite/rusqlite) | 0.40.2 | MIT | Copyright (c) 2014 The rusqlite developers |
| [`rustc_version`](https://github.com/djc/rustc-version-rs) | 0.4.1 | MIT OR Apache-2.0 | Copyright (c) 2016 The Rust Project Developers |
| [`rustc-hash`](https://github.com/rust-lang/rustc-hash) | 2.1.3 | Apache-2.0 OR MIT | — |
| [`rustls`](https://github.com/rustls/rustls) | 0.23.45 | Apache-2.0 OR ISC OR MIT | Copyright (c) 2016, Joseph Birr-Pixton <jpixton@gmail.com><br>Copyright (c) 2016 Joseph Birr-Pixton <jpixton@gmail.com> |
| [`rustls-pki-types`](https://github.com/rustls/pki-types) | 1.15.1 | MIT OR Apache-2.0 | Copyright 2023 Dirkjan Ochtman<br>Copyright (c) 2023 Dirkjan Ochtman <dirkjan@ochtman.nl> |
| [`rustls-webpki`](https://github.com/rustls/webpki) | 0.103.15 | ISC | Copyright 2015 Brian Smith. |
| [`rustversion`](https://github.com/dtolnay/rustversion) | 1.0.23 | MIT OR Apache-2.0 | — |
| [`same-file`](https://github.com/BurntSushi/same-file) | 1.0.6 | Unlicense/MIT | Copyright (c) 2017 Andrew Gallant |
| [`schemars`](https://github.com/GREsau/schemars) | 0.8.22 | MIT | Copyright (c) 2019 Graham Esau |
| [`schemars`](https://github.com/GREsau/schemars) | 0.9.0 | MIT | Copyright (c) 2019 Graham Esau |
| [`schemars`](https://github.com/GREsau/schemars) | 1.2.2 | MIT | Copyright (c) 2019 Graham Esau |
| [`schemars_derive`](https://github.com/GREsau/schemars) | 0.8.22 | MIT | Copyright (c) 2019 Graham Esau |
| [`scopeguard`](https://github.com/bluss/scopeguard) | 1.2.0 | MIT OR Apache-2.0 | Copyright (c) 2016-2019 Ulrik Sverdrup "bluss" and scopeguard developers |
| [`selectors`](https://github.com/servo/stylo) | 0.38.0 | MPL-2.0 | — |
| [`semver`](https://github.com/dtolnay/semver) | 1.0.28 | MIT OR Apache-2.0 | — |
| [`serde`](https://github.com/serde-rs/serde) | 1.0.229 | MIT OR Apache-2.0 | — |
| [`serde_core`](https://github.com/serde-rs/serde) | 1.0.229 | MIT OR Apache-2.0 | — |
| [`serde_derive`](https://github.com/serde-rs/serde) | 1.0.229 | MIT OR Apache-2.0 | — |
| [`serde_derive_internals`](https://github.com/serde-rs/serde) | 0.29.1 | MIT OR Apache-2.0 | — |
| [`serde_json`](https://github.com/serde-rs/json) | 1.0.151 | MIT OR Apache-2.0 | — |
| [`serde_repr`](https://github.com/dtolnay/serde-repr) | 0.1.21 | MIT OR Apache-2.0 | — |
| [`serde_spanned`](https://github.com/toml-rs/toml) | 0.6.9 | MIT OR Apache-2.0 | Copyright (c) Individual contributors |
| [`serde_spanned`](https://github.com/toml-rs/toml) | 1.1.2 | MIT OR Apache-2.0 | Copyright Individual contributors<br>Copyright (c) Individual contributors |
| [`serde_with`](https://github.com/jonasbb/serde_with/) | 3.24.0 | MIT OR Apache-2.0 | Copyright (c) 2015 |
| [`serde_with_macros`](https://github.com/jonasbb/serde_with/) | 3.24.0 | MIT OR Apache-2.0 | Copyright (c) 2015 |
| [`serde-untagged`](https://github.com/dtolnay/serde-untagged) | 0.1.9 | MIT OR Apache-2.0 | — |
| [`serialize-to-javascript`](https://github.com/chippers/serialize-to-javascript) | 0.1.2 | MIT OR Apache-2.0 | Copyright (c) 2021 Chip Reed |
| [`serialize-to-javascript-impl`](https://github.com/chippers/serialize-to-javascript) | 0.1.2 | MIT OR Apache-2.0 | Copyright (c) 2021 Chip Reed |
| [`servo_arc`](https://github.com/servo/stylo) | 0.4.3 | MIT OR Apache-2.0 | — |
| [`sha2`](https://github.com/RustCrypto/hashes) | 0.10.9 | MIT OR Apache-2.0 | Copyright (c) 2006-2009 Graydon Hoare<br>Copyright (c) 2009-2013 Mozilla Foundation<br>Copyright (c) 2016 Artyom Pavlov |
| [`shlex`](https://github.com/comex/rust-shlex) | 2.0.1 | MIT OR Apache-2.0 | Copyright 2015 Nicholas Allegra (comex).<br>Copyright (c) 2015 Nicholas Allegra (comex). |
| [`simd-adler32`](https://github.com/mcountryman/simd-adler32) | 0.3.10 | MIT | Copyright (c) [2021] [Marvin Countryman] |
| [`simdutf8`](https://github.com/rusticstuff/simdutf8) | 0.1.5 | MIT OR Apache-2.0 | — |
| [`siphasher`](https://github.com/jedisct1/rust-siphash) | 1.0.4 | MIT OR Apache-2.0 | Copyright 2012-2016 The Rust Project Developers.<br>Copyright 2016-2026 Frank Denis. |
| [`slab`](https://github.com/tokio-rs/slab) | 0.4.12 | MIT | Copyright (c) 2019 Carl Lerche |
| [`smallvec`](https://github.com/servo/rust-smallvec) | 1.16.2 | MIT OR Apache-2.0 | Copyright (c) 2018 The Servo Project Developers |
| [`socket2`](https://github.com/rust-lang/socket2) | 0.6.5 | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton |
| [`softbuffer`](https://github.com/rust-windowing/softbuffer) | 0.4.8 | MIT OR Apache-2.0 | Copyright 2022 Kirill Chibisov |
| [`soup3`](https://gitlab.gnome.org/World/Rust/soup3-rs) | 0.5.0 | MIT | Copyright (c) 2013-2017, The Gtk-rs Project Developers. |
| [`soup3-sys`](https://gitlab.gnome.org/World/Rust/soup3-rs) | 0.5.0 | MIT | Copyright (c) 2013-2017, The Gtk-rs Project Developers. |
| [`sqlite-wasm-rs`](https://github.com/Spxg/sqlite-wasm-rs) | 0.5.5 | MIT | Copyright (c) 2024 Spxg |
| [`stable_deref_trait`](https://github.com/storyyeller/stable_deref_trait) | 1.2.1 | MIT OR Apache-2.0 | Copyright (c) 2017 Robert Grosse |
| [`string_cache`](https://github.com/servo/string-cache) | 0.9.0 | MIT OR Apache-2.0 | Copyright (c) 2012-2013 Mozilla Foundation |
| [`string_cache_codegen`](https://github.com/servo/string-cache) | 0.6.1 | MIT OR Apache-2.0 | Copyright (c) 2012-2013 Mozilla Foundation |
| [`strsim`](https://github.com/rapidfuzz/strsim-rs) | 0.11.1 | MIT | Copyright (c) 2015 Danny Guo<br>Copyright (c) 2016 Titus Wormer <tituswormer@gmail.com><br>Copyright (c) 2018 Akash Kurdekar |
| [`subtle`](https://github.com/dalek-cryptography/subtle) | 2.6.1 | BSD-3-Clause | Copyright (c) 2016-2017 Isis Agora Lovecruft, Henry de Valence. All rights reserved.<br>Copyright (c) 2016-2024 Isis Agora Lovecruft. All rights reserved. |
| [`swift-rs`](https://github.com/Brendonovich/swift-rs) | 1.0.8 | MIT OR Apache-2.0 | Copyright 2023 The swift-rs developers<br>Copyright (c) 2023 The swift-rs Developers |
| [`syn`](https://github.com/dtolnay/syn) | 1.0.109 | MIT OR Apache-2.0 | — |
| [`syn`](https://github.com/dtolnay/syn) | 2.0.119 | MIT OR Apache-2.0 | — |
| [`syn`](https://github.com/dtolnay/syn) | 3.0.6 | MIT OR Apache-2.0 | — |
| [`sync_wrapper`](https://github.com/Actyx/sync_wrapper) | 1.0.2 | Apache-2.0 | — |
| [`synstructure`](https://github.com/mystor/synstructure) | 0.14.0 | MIT | Copyright 2016 Nika Layzell |
| [`system-deps`](https://github.com/gdesmott/system-deps) | 6.2.2 | MIT OR Apache-2.0 | — |
| [`tao`](https://github.com/tauri-apps/tao) | 0.37.1 | Apache-2.0 | — |
| [`tao-macros`](https://github.com/tauri-apps/tao) | 0.1.4 | MIT OR Apache-2.0 | Copyright (c) 2022 - Present Tauri Apps Contributors |
| [`target-lexicon`](https://github.com/bytecodealliance/target-lexicon) | 0.12.16 | Apache-2.0 WITH LLVM-exception | — |
| [`tauri`](https://github.com/tauri-apps/tauri) | 2.12.1 | Apache-2.0 OR MIT | Copyright (c) 2017 - Present Tauri Apps Contributors |
| [`tauri-build`](https://github.com/tauri-apps/tauri) | 2.7.1 | Apache-2.0 OR MIT | Copyright (c) 2017 - Present Tauri Apps Contributors |
| [`tauri-codegen`](https://github.com/tauri-apps/tauri) | 2.7.1 | Apache-2.0 OR MIT | Copyright (c) 2017 - Present Tauri Apps Contributors |
| [`tauri-macros`](https://github.com/tauri-apps/tauri) | 2.7.1 | Apache-2.0 OR MIT | Copyright (c) 2017 - Present Tauri Apps Contributors |
| [`tauri-plugin`](https://github.com/tauri-apps/tauri) | 2.7.1 | Apache-2.0 OR MIT | — |
| [`tauri-plugin-log`](https://github.com/tauri-apps/plugins-workspace) | 2.10.0 | Apache-2.0 OR MIT | Copyright (c) 2017 - Present Tauri Apps Contributors |
| [`tauri-runtime`](https://github.com/tauri-apps/tauri) | 2.12.1 | Apache-2.0 OR MIT | Copyright (c) 2017 - Present Tauri Apps Contributors |
| [`tauri-runtime-wry`](https://github.com/tauri-apps/tauri) | 2.12.1 | Apache-2.0 OR MIT | Copyright (c) 2017 - Present Tauri Apps Contributors |
| [`tauri-utils`](https://github.com/tauri-apps/tauri) | 2.10.1 | Apache-2.0 OR MIT | Copyright (c) 2017 - Present Tauri Apps Contributors |
| [`tauri-winres`](https://github.com/tauri-apps/winres) | 0.3.6 | MIT | Copyright (c) 2023 - Present Tauri Apps Contributors<br>Copyright (c) 2016 Max Resch |
| [`tendril`](https://github.com/servo/html5ever) | 0.5.1 | MIT OR Apache-2.0 | Copyright (c) 2015 Keegan McAllister |
| [`thiserror`](https://github.com/dtolnay/thiserror) | 1.0.69 | MIT OR Apache-2.0 | — |
| [`thiserror`](https://github.com/dtolnay/thiserror) | 2.0.21 | MIT OR Apache-2.0 | — |
| [`thiserror-impl`](https://github.com/dtolnay/thiserror) | 1.0.69 | MIT OR Apache-2.0 | — |
| [`thiserror-impl`](https://github.com/dtolnay/thiserror) | 2.0.21 | MIT OR Apache-2.0 | — |
| [`time`](https://github.com/time-rs/time) | 0.3.55 | MIT OR Apache-2.0 | Copyright (c) Jacob Pratt et al. |
| [`time-core`](https://github.com/time-rs/time) | 0.1.9 | MIT OR Apache-2.0 | Copyright (c) Jacob Pratt et al. |
| [`time-macros`](https://github.com/time-rs/time) | 0.2.32 | MIT OR Apache-2.0 | Copyright (c) Jacob Pratt et al. |
| [`tinystr`](https://github.com/unicode-org/icu4x) | 0.8.4 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| [`tinyvec`](https://github.com/Lokathor/tinyvec) | 1.13.3 | Zlib OR Apache-2.0 OR MIT | Copyright (c) 2019 Daniel "Lokathor" Gee. |
| [`tokio`](https://github.com/tokio-rs/tokio) | 1.53.2 | MIT | Copyright (c) Tokio Contributors |
| [`tokio-macros`](https://github.com/tokio-rs/tokio) | 2.7.2 | MIT | Copyright (c) 2019 Yoshua Wuyts<br>Copyright (c) Tokio Contributors |
| [`tokio-util`](https://github.com/tokio-rs/tokio) | 0.6.10 | MIT | Copyright (c) 2021 Tokio Contributors |
| [`tokio-util`](https://github.com/tokio-rs/tokio) | 0.7.19 | MIT | Copyright (c) Tokio Contributors |
| [`toml`](https://github.com/toml-rs/toml) | 0.8.2 | MIT OR Apache-2.0 | Copyright (c) Individual contributors |
| [`toml`](https://github.com/toml-rs/toml) | 1.1.7+spec-1.1.0 | MIT OR Apache-2.0 | Copyright Individual contributors<br>Copyright (c) Individual contributors |
| [`toml_datetime`](https://github.com/toml-rs/toml) | 0.6.3 | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton |
| [`toml_datetime`](https://github.com/toml-rs/toml) | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 | Copyright Individual contributors<br>Copyright (c) Individual contributors |
| [`toml_edit`](https://github.com/toml-rs/toml) | 0.19.15 | MIT OR Apache-2.0 | Copyright (c) Individual contributors |
| [`toml_edit`](https://github.com/toml-rs/toml) | 0.20.2 | MIT OR Apache-2.0 | Copyright (c) Individual contributors |
| [`toml_edit`](https://github.com/toml-rs/toml) | 0.25.16+spec-1.1.0 | MIT OR Apache-2.0 | Copyright Individual contributors<br>Copyright (c) Individual contributors |
| [`toml_parser`](https://github.com/toml-rs/toml) | 1.1.4+spec-1.1.0 | MIT OR Apache-2.0 | Copyright Individual contributors<br>Copyright (c) Individual contributors |
| [`toml_writer`](https://github.com/toml-rs/toml) | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 | Copyright Individual contributors<br>Copyright (c) Individual contributors |
| [`tower`](https://github.com/tower-rs/tower) | 0.5.3 | MIT | Copyright (c) 2019 Tower Contributors |
| [`tower-http`](https://github.com/tower-rs/tower-http) | 0.6.11 | MIT | Copyright (c) 2019-2021 Tower Contributors |
| [`tower-layer`](https://github.com/tower-rs/tower) | 0.3.3 | MIT | Copyright (c) 2019 Tower Contributors |
| [`tower-service`](https://github.com/tower-rs/tower) | 0.3.3 | MIT | Copyright (c) 2019 Tower Contributors |
| [`tracing`](https://github.com/tokio-rs/tracing) | 0.1.44 | MIT | Copyright (c) 2019 Tokio Contributors |
| [`tracing-core`](https://github.com/tokio-rs/tracing) | 0.1.36 | MIT | Copyright (c) 2019 Tokio Contributors |
| [`tray-icon`](https://github.com/tauri-apps/tray-icon) | 0.25.1 | MIT OR Apache-2.0 | Copyright (c) 2022-2022 Tauri Programme within The Commons Conservancy |
| [`try-lock`](https://github.com/seanmonstar/try-lock) | 0.2.5 | MIT | Copyright (c) 2018-2023 Sean McArthur<br>Copyright (c) 2016 Alex Crichton |
| [`typeid`](https://github.com/dtolnay/typeid) | 1.0.3 | MIT OR Apache-2.0 | — |
| [`typenum`](https://github.com/paholg/typenum) | 1.20.1 | MIT OR Apache-2.0 | Copyright 2014 Paho Lurie-Gregg<br>Copyright (c) 2014 Paho Lurie-Gregg |
| [`unicode-ident`](https://github.com/dtolnay/unicode-ident) | 1.0.26 | (MIT OR Apache-2.0) AND Unicode-3.0 | Copyright © 1991-2023 Unicode, Inc. |
| [`unicode-segmentation`](https://github.com/unicode-rs/unicode-segmentation) | 1.13.3 | MIT OR Apache-2.0 | Copyright (c) 2015 The Rust Project Developers |
| [`universal-hash`](https://github.com/RustCrypto/traits) | 0.5.1 | MIT OR Apache-2.0 | Copyright (c) 2019-2020 RustCrypto Developers |
| [`unix_mode`](https://github.com/sourcefrog/unix_mode) | 0.1.4 | Apache-2.0 | — |
| [`untrusted`](https://github.com/briansmith/untrusted) | 0.9.0 | ISC | Copyright 2015-2016 Brian Smith. |
| [`ureq`](https://github.com/algesten/ureq) | 3.4.2 | MIT OR Apache-2.0 | Copyright (c) 2019 Martin Algesten |
| [`ureq-proto`](https://github.com/algesten/ureq-proto) | 0.6.4 | MIT OR Apache-2.0 | Copyright 2022 Martin Algesten |
| [`url`](https://github.com/servo/rust-url) | 2.5.8 | MIT OR Apache-2.0 | Copyright (c) 2013-2025 The rust-url developers |
| [`urlpattern`](https://github.com/denoland/rust-urlpattern) | 0.6.0 | MIT | Copyright (c) 2021 the Deno authors |
| [`utf8_iter`](https://github.com/hsivonen/utf8_iter) | 1.0.4 | Apache-2.0 OR MIT | Copyright Mozilla Foundation |
| [`utf8-zero`](https://github.com/algesten/utf8-zero) | 0.8.1 | MIT OR Apache-2.0 | — |
| [`uuid`](https://github.com/uuid-rs/uuid) | 1.27.0 | Apache-2.0 OR MIT | Copyright (c) 2014 The Rust Project Developers<br>Copyright (c) 2018 Ashley Mannix, Christopher Armstrong, Dylan DPC, Hunar Roop Kahlon |
| [`vcpkg`](https://github.com/mcgoo/vcpkg-rs) | 0.2.15 | MIT/Apache-2.0 | Copyright (c) 2017 Jim McGrath |
| [`version_check`](https://github.com/SergioBenitez/version_check) | 0.9.5 | MIT/Apache-2.0 | Copyright (c) 2017-2018 Sergio Benitez |
| [`version-compare`](https://gitlab.com/timvisee/version-compare) | 0.2.1 | MIT | Copyright (c) 2017 Tim Visée |
| [`vswhom`](https://github.com/nabijaczleweli/vswhom.rs) | 0.1.0 | MIT | Copyright (c) 2019 nabijaczleweli |
| [`vswhom-sys`](https://github.com/nabijaczleweli/vswhom-sys.rs) | 0.1.3 | MIT | Copyright (c) 2019 nabijaczleweli |
| [`walkdir`](https://github.com/BurntSushi/walkdir) | 2.5.0 | Unlicense/MIT | Copyright (c) 2015 Andrew Gallant |
| [`want`](https://github.com/seanmonstar/want) | 0.3.2 | MIT | Copyright (c) 2018-2019 Sean McArthur |
| [`wasi`](https://github.com/bytecodealliance/wasi) | 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | — |
| [`wasip2`](https://github.com/bytecodealliance/wasi-rs) | 1.0.4+wasi-0.2.12 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | — |
| [`wasm-bindgen`](https://github.com/wasm-bindgen/wasm-bindgen) | 0.2.129 | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton |
| [`wasm-bindgen-futures`](https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/futures) | 0.4.79 | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton |
| [`wasm-bindgen-macro`](https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/macro) | 0.2.129 | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton |
| [`wasm-bindgen-macro-support`](https://github.com/wasm-bindgen/wasm-bindgen/tree/main/crates/macro-support) | 0.2.129 | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton |
| [`wasm-bindgen-shared`](https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/shared) | 0.2.129 | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton |
| [`wasm-streams`](https://github.com/MattiasBuelens/wasm-streams/) | 0.5.0 | MIT OR Apache-2.0 | — |
| [`web_atoms`](https://github.com/servo/html5ever) | 0.2.6 | MIT OR Apache-2.0 | Copyright (c) 2014 The html5ever Project Developers |
| [`web-sys`](https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/web-sys) | 0.3.106 | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton |
| [`web-time`](https://github.com/daxpedda/web-time) | 1.1.0 | MIT OR Apache-2.0 | Copyright 2023 dAxpeDDa<br>Copyright (c) 2023 dAxpeDDa |
| [`webkit2gtk`](https://github.com/tauri-apps/webkit2gtk-rs) | 2.0.2 | MIT | Copyright (c) 2016 Boucher, Antoni <bouanto@zoho.com><br>Copyright (c) 2017-2021, The Gtk-rs Project Developers.<br>Copyright (c) 2021, Tauri Programme within The Commons Conservancy |
| [`webkit2gtk-sys`](https://github.com/tauri-apps/webkit2gtk-rs) | 2.0.2 | MIT | Copyright (c) 2016 Boucher, Antoni <bouanto@zoho.com> |
| [`webpki-roots`](https://github.com/rustls/webpki-roots) | 1.0.9 | CDLA-Permissive-2.0 | — |
| [`webview2-com`](https://github.com/wravery/webview2-rs) | 0.39.1 | MIT | — |
| [`webview2-com-macros`](https://github.com/wravery/webview2-rs) | 0.8.1 | MIT | — |
| [`webview2-com-sys`](https://github.com/wravery/webview2-rs) | 0.39.1 | MIT | — |
| [`winapi`](https://github.com/retep998/winapi-rs) | 0.3.9 | MIT/Apache-2.0 | Copyright (c) 2015-2018 The winapi-rs Developers |
| [`winapi-i686-pc-windows-gnu`](https://github.com/retep998/winapi-rs) | 0.4.0 | MIT/Apache-2.0 | — |
| [`winapi-util`](https://github.com/BurntSushi/winapi-util) | 0.1.11 | Unlicense OR MIT | Copyright (c) 2017 Andrew Gallant |
| [`winapi-x86_64-pc-windows-gnu`](https://github.com/retep998/winapi-rs) | 0.4.0 | MIT/Apache-2.0 | — |
| [`window-vibrancy`](https://github.com/tauri-apps/tauri-plugin-vibrancy) | 0.8.1 | Apache-2.0 OR MIT | Copyright (c) 2020-2022 Tauri Programme within The Commons Conservancy |
| [`windows`](https://github.com/microsoft/windows-rs) | 0.62.2 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows_aarch64_gnullvm`](https://github.com/microsoft/windows-rs) | 0.42.2 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows_aarch64_gnullvm`](https://github.com/microsoft/windows-rs) | 0.52.6 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows_aarch64_msvc`](https://github.com/microsoft/windows-rs) | 0.42.2 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows_aarch64_msvc`](https://github.com/microsoft/windows-rs) | 0.52.6 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows_i686_gnu`](https://github.com/microsoft/windows-rs) | 0.42.2 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows_i686_gnu`](https://github.com/microsoft/windows-rs) | 0.52.6 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows_i686_gnullvm`](https://github.com/microsoft/windows-rs) | 0.52.6 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows_i686_msvc`](https://github.com/microsoft/windows-rs) | 0.42.2 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows_i686_msvc`](https://github.com/microsoft/windows-rs) | 0.52.6 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows_x86_64_gnu`](https://github.com/microsoft/windows-rs) | 0.42.2 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows_x86_64_gnu`](https://github.com/microsoft/windows-rs) | 0.52.6 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows_x86_64_gnullvm`](https://github.com/microsoft/windows-rs) | 0.42.2 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows_x86_64_gnullvm`](https://github.com/microsoft/windows-rs) | 0.52.6 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows_x86_64_msvc`](https://github.com/microsoft/windows-rs) | 0.42.2 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows_x86_64_msvc`](https://github.com/microsoft/windows-rs) | 0.52.6 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows-collections`](https://github.com/microsoft/windows-rs) | 0.3.2 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows-core`](https://github.com/microsoft/windows-rs) | 0.62.2 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows-future`](https://github.com/microsoft/windows-rs) | 0.3.2 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows-implement`](https://github.com/microsoft/windows-rs) | 0.60.2 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows-interface`](https://github.com/microsoft/windows-rs) | 0.59.3 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows-link`](https://github.com/microsoft/windows-rs) | 0.2.1 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows-numerics`](https://github.com/microsoft/windows-rs) | 0.3.1 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows-result`](https://github.com/microsoft/windows-rs) | 0.4.1 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows-strings`](https://github.com/microsoft/windows-rs) | 0.5.1 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows-sys`](https://github.com/microsoft/windows-rs) | 0.45.0 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows-sys`](https://github.com/microsoft/windows-rs) | 0.52.0 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows-sys`](https://github.com/microsoft/windows-rs) | 0.61.2 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows-targets`](https://github.com/microsoft/windows-rs) | 0.42.2 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows-targets`](https://github.com/microsoft/windows-rs) | 0.52.6 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows-threading`](https://github.com/microsoft/windows-rs) | 0.2.1 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`windows-version`](https://github.com/microsoft/windows-rs) | 0.1.7 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| [`winnow`](https://github.com/winnow-rs/winnow) | 0.5.40 | MIT | — |
| [`winnow`](https://github.com/winnow-rs/winnow) | 1.0.4 | MIT | — |
| [`winreg`](https://github.com/gentoo90/winreg-rs) | 0.56.0 | MIT | Copyright (c) 2015 Igor Shaula |
| [`wit-bindgen`](https://github.com/bytecodealliance/wit-bindgen) | 0.57.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | — |
| [`writeable`](https://github.com/unicode-org/icu4x) | 0.6.4 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| [`wry`](https://github.com/tauri-apps/wry) | 0.57.0 | Apache-2.0 OR MIT | Copyright (c) 2020-2023 Ngo Iok Ui & Tauri Programme within The Commons Conservancy |
| [`x11`](https://github.com/AltF02/x11-rs) | 2.21.0 | MIT | — |
| [`x11-dl`](https://github.com/AltF02/x11-rs) | 2.21.0 | MIT | — |
| [`yoke`](https://github.com/unicode-org/icu4x) | 0.8.3 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| [`yoke-derive`](https://github.com/unicode-org/icu4x) | 0.8.4 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| [`zerofrom`](https://github.com/unicode-org/icu4x) | 0.1.8 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| [`zerofrom-derive`](https://github.com/unicode-org/icu4x) | 0.1.8 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| [`zeroize`](https://github.com/RustCrypto/utils) | 1.9.1 | Apache-2.0 OR MIT | Copyright (c) 2018-2026 The RustCrypto Project Developers |
| [`zerotrie`](https://github.com/unicode-org/icu4x) | 0.2.5 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| [`zerovec`](https://github.com/unicode-org/icu4x) | 0.11.8 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| [`zerovec-derive`](https://github.com/unicode-org/icu4x) | 0.11.6 | Unicode-3.0 | Copyright © 2020-2024 Unicode, Inc. |
| [`zlib-rs`](https://github.com/trifectatechfoundation/zlib-rs) | 0.6.8 | Zlib | — |
| [`zmij`](https://github.com/dtolnay/zmij) | 1.0.23 | MIT | — |

## Pacotes npm (package-lock.json)

36 pacotes instalados em `node_modules`, incluindo ferramentas de
desenvolvimento e de build. Quando um pacote não distribui um arquivo de
licença, é usada a licença declarada em seu `package.json` e nenhuma linha de
copyright separada é listada.

| Package | Version | License | Copyright notice |
| --- | --- | --- | --- |
| [`@floating-ui/core`](https://github.com/floating-ui/floating-ui) | 1.8.0 | MIT | Copyright (c) 2021-present Floating UI contributors |
| [`@floating-ui/dom`](https://github.com/floating-ui/floating-ui) | 1.8.0 | MIT | Copyright (c) 2021-present Floating UI contributors |
| [`@floating-ui/utils`](https://github.com/floating-ui/floating-ui) | 0.2.12 | MIT | Copyright (c) 2021-present Floating UI contributors |
| [`@govbr-ds/core`](gitlab:govbr-ds/govbr-ds-dev/govbr-ds-dev-core) | 3.7.0 | MIT | Copyright (c) 2022 SERPRO |
| [`@govbr-ds/webcomponents`](https://gitlab.com/govbr-ds/bibliotecas/wbc/govbr-ds-wbc) | 2.2.0 | MIT | Copyright (c) 2022 SERPRO |
| [`@iconify/types`](https://github.com/iconify/iconify) | 2.0.0 | MIT | Copyright (c) 2021 - 2022 Vjacheslav Trushkin / Iconify OÜ |
| [`@oxc-project/types`](https://github.com/oxc-project/oxc) | 0.153.0 | MIT | Copyright (c) 2024-present VoidZero Inc. & Contributors<br>Copyright (c) 2023 Boshen |
| [`@popperjs/core`](github:popperjs/popper-core) | 2.11.8 | MIT | Copyright (c) 2019 Federico Zivolo |
| [`@rolldown/binding-win32-x64-msvc`](https://github.com/rolldown/rolldown) | 1.2.13 | MIT | — |
| [`@rolldown/pluginutils`](https://github.com/rolldown/plugins) | 1.0.1 | MIT | Copyright (c) 2026-present, rolldown/plugins repository contributors |
| [`@tauri-apps/api`](https://github.com/tauri-apps/tauri) | 2.12.1 | Apache-2.0 OR MIT | Copyright (c) 2017 - Present Tauri Apps Contributors |
| [`@tauri-apps/cli`](https://github.com/tauri-apps/tauri) | 2.12.1 | Apache-2.0 OR MIT | Copyright (c) 2017 - Present Tauri Apps Contributors |
| [`@tauri-apps/cli-win32-x64-msvc`](https://github.com/tauri-apps/tauri) | 2.12.1 | Apache-2.0 OR MIT | — |
| [`@types/react`](https://github.com/DefinitelyTyped/DefinitelyTyped) | 19.3.0 | MIT | Copyright (c) Microsoft Corporation. |
| [`@types/react-dom`](https://github.com/DefinitelyTyped/DefinitelyTyped) | 19.3.0 | MIT | Copyright (c) Microsoft Corporation. |
| [`@vitejs/plugin-react`](https://github.com/vitejs/vite-plugin-react) | 6.1.2 | MIT | Copyright (c) 2019-present, Yuxi (Evan) You and Vite contributors |
| [`csstype`](https://github.com/frenic/csstype) | 3.2.3 | MIT | Copyright (c) 2017-2018 Fredrik Nicol |
| [`date-fns`](https://github.com/date-fns/date-fns) | 4.4.0 | MIT | Copyright (c) 2021 Sasha Koss and Lesha Koss https://kossnocorp.mit-license.org |
| [`detect-libc`](https://github.com/lovell/detect-libc) | 2.1.2 | Apache-2.0 | — |
| [`fdir`](https://github.com/thecodrr/fdir) | 6.5.0 | MIT | Copyright 2023 Abdullah Atta |
| [`flatpickr`](https://github.com/chmln/flatpickr) | 4.6.13 | MIT | Copyright (c) 2017 Gregory Petrosyan |
| [`iconify-icon`](https://github.com/iconify/iconify) | 3.0.2 | MIT | Copyright (c) 2022-PRESENT Vjacheslav Trushkin |
| [`lightningcss`](https://github.com/parcel-bundler/lightningcss) | 1.33.0 | MPL-2.0 | — |
| [`lightningcss-win32-x64-msvc`](https://github.com/parcel-bundler/lightningcss) | 1.33.0 | MPL-2.0 | — |
| [`nanoid`](ai/nanoid) | 3.3.20 | MIT | Copyright 2017 Andrey Sitnik <andrey@sitnik.ru> |
| [`picocolors`](alexeyraspopov/picocolors) | 1.1.1 | ISC | Copyright (c) 2021-2024 Oleksii Raspopov, Kostiantyn Denysov, Anton Verinov |
| [`picomatch`](micromatch/picomatch) | 4.0.7 | MIT | Copyright (c) 2017-present, Jon Schlinkert. |
| [`postcss`](postcss/postcss) | 8.5.29 | MIT | Copyright 2013 Andrey Sitnik <andrey@sitnik.es> |
| [`react`](https://github.com/react/react) | 19.3.0 | MIT | Copyright (c) Meta Platforms, Inc. and affiliates. |
| [`react-dom`](https://github.com/react/react) | 19.3.0 | MIT | Copyright (c) Meta Platforms, Inc. and affiliates. |
| [`rolldown`](https://github.com/rolldown/rolldown) | 1.2.13 | MIT | Copyright (c) 2024-present VoidZero Inc. & Contributors |
| [`scheduler`](https://github.com/react/react) | 0.28.0 | MIT | Copyright (c) Meta Platforms, Inc. and affiliates. |
| [`source-map-js`](7rulnik/source-map-js) | 1.2.2 | BSD-3-Clause | Copyright (c) 2009-2011, Mozilla Foundation and contributors |
| [`tinyglobby`](https://github.com/SuperchupuDev/tinyglobby) | 0.2.17 | MIT | Copyright (c) 2024 Madeline Gurriarán |
| [`typescript`](https://github.com/microsoft/TypeScript) | 5.9.3 | Apache-2.0 | — |
| [`vite`](https://github.com/vitejs/vite) | 8.3.4 | MIT | Copyright (c) 2019-present, VoidZero Inc. and Vite contributors |

## Observações sobre licenças específicas

- **Componentes com dupla ou múltiplas licenças** (por exemplo
  `MIT OR Apache-2.0`). A tabela reproduz a expressão de licença exata
  declarada por cada componente. Quando a expressão oferece escolha, esta
  distribuição utiliza a opção MIT sempre que o MIT é uma das alternativas;
  componentes disponíveis apenas sob outra licença são usados nos termos neles
  indicados. Você pode optar por qualquer outra alternativa oferecida pelo
  componente.
- **Componentes Apache-2.0.** O texto completo da Licença Apache 2.0, incluindo
  sua concessão de patentes e requisitos de atribuição, está em
  [`licenses/Apache-2.0.txt`](licenses/Apache-2.0.txt). Nenhum arquivo NOTICE
  desses projetos é exigido além das linhas de copyright reproduzidas aqui.
- **Componentes Apache-2.0 WITH LLVM-exception** (`target-lexicon`, `wasi`,
  `wasip2`, `wit-bindgen`). São dependências relacionadas a WebAssembly/UEFI.
  Sua licença é a Licença Apache 2.0 com a exceção LLVM, reproduzida em
  [`licenses/Apache-2.0-LLVM-exception.txt`](licenses/Apache-2.0-LLVM-exception.txt).
- **Alternativa LGPL-2.1-or-later** (`r-efi`). Este crate é oferecido sob
  `MIT OR Apache-2.0 OR LGPL-2.1-or-later`; esta distribuição utiliza a opção
  MIT, portanto os termos da LGPL não se aplicam a ele.
- **Componentes MPL-2.0** (`cssparser`, `cssparser-macros`, `dtoa-short`,
  `option-ext`, `selectors`, `lightningcss`). São usados sem modificação como
  dependências. O código-fonte de cada componente está disponível no
  repositório indicado na tabela e em <https://crates.io> / <https://npmjs.com>
  pelo nome e versão do pacote.
- **Componentes Unicode-3.0** fazem parte do ICU4X e usam a Licença Unicode V3,
  reproduzida em [`licenses/Unicode-3.0.txt`](licenses/Unicode-3.0.txt).
- **Componentes CC0-1.0 / Unlicense / 0BSD / MIT-0** são dedicações ao domínio
  público ou permissões equivalentes que não exigem atribuição; estão listados
  por completude e transparência.
- **Ferramentas exclusivas de desenvolvimento** (Vite, Rolldown, TypeScript,
  Tauri CLI, definições de tipos e semelhantes) não fazem parte do aplicativo
  distribuído, mas seus termos de licença são reproduzidos aqui porque os
  pacotes são usados para construir os binários distribuídos.
- **Biblioteca padrão e toolchain do Rust** são cobertas por suas próprias
  licenças (MIT OR Apache-2.0) e não são redistribuídas como componentes
  separados deste projeto.

## Como relatar uma omissão

Se você acredita que um componente está ausente, atribuído incorretamente ou
acompanhado do texto de licença errado, abra uma issue no repositório do
projeto para que este documento seja corrigido. Este arquivo é mantido
manualmente a partir dos lockfiles; ele não é gerado durante o build.

---

O Legisladoc Sync e todos os componentes de terceiros são fornecidos "COMO
ESTÁ" (AS IS), sem garantia de qualquer tipo. Consulte [LICENSE.pt-BR](LICENSE.pt-BR)
para a licença MIT completa e o aviso adicional de uso "COMO ESTÁ".
