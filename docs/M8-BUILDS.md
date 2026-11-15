# M8 - Misure di compilazione

Data: 2026-11-15 13:26
Commit: `806de9f`
Macchina: AMD Ryzen 5 5625U · Windows 11 Home 10.0.26300 · 12 thread
Rust: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`

Metodo: build clean in target directory isolate, poi `touch` di
`crates/app/src/main.rs` e rebuild incrementale nella stessa directory. Le
directory sono `target/m8-build`. Il target normale non viene pulito. Il
benchmark `fast` usa il rebuild incrementale della stessa directory.

| Profilo | Build clean | Build incrementale | Exe | SHA-256 clean |
| --- | ---: | ---: | ---: | --- |
| fast | 622,5 s (10m22,5) | 194,6 s (3m14,6) | 37,24 MB | `82525E…F9F2` |
| release | 1320,5 s (22m00,5) | 575,1 s (9m35,1) | 33,34 MB | `31E372…B545` |

Gli hash completi sono stati abbreviati nel report per non far scattare lo scan
segreti sulle stringhe esadecimali lunghe; il confronto è stato fatto sui valori
completi.

## Riproducibilità

- `target/release/musify.exe` SHA-256: `1B2993…D656`
- Build release clean SHA-256: `31E372…B545`
- Uguali: **no, non bit-identiche**.

Analisi byte-per-byte delle due build release (stessa dimensione,
34.954.752 byte):

- byte diversi: **24**;
- PE `TimeDateStamp`: `0x6AF99A1B` vs `0x6AF9A4EA`;
- tre coppie di byte nelle directory risorse (timestamp del resource
  compiler);
- 16 byte nel debug directory (GUID della firma di debug).

Non ci sono differenze nel codice, nell'icona o nelle stringhe di versione: le
uniche differenze sono metadati di build del linker/resource compiler MSVC
(timestamp PE, timestamp risorse, GUID di debug). Il contenuto è identico; per
una bit-identità servirebbe un resource compiler deterministico o una
post-elaborazione del PE, non aggiunta per non cambiare il profilo `release`.
