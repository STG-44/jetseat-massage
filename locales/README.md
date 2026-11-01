# Localization catalog

`messages.json` is embedded in the executable. `source` retains the original Russian phrase; `text` contains English, German, French, Spanish, Brazilian Portuguese, Italian, Simplified Chinese, Japanese, Korean, and Ukrainian in that order.

Source templates may contain Rust formatting fields. Translations use numeric placeholders (`{0}`, `{1}`, …), preserving every field once. Values are formatted before translation; nested application errors are translated recursively while native OS errors and device names remain unchanged.

The UI translates at render time, so changing language also updates existing status messages. The selected language is stored with normal app settings. The initial language comes from the operating system, with English as the fallback. CJK font faces follow Japanese/Korean/Simplified Chinese preferences.

Run `cargo test` to validate placeholder coverage and runtime formatting. Translations should be reviewed by native speakers as terminology evolves.
