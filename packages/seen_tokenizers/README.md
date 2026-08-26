# seen_tokenizers

`seen_tokenizers` provides bounded, deterministic tokenizer foundations in
native Seen. Version 0.1.0 supports strict Hugging Face byte-BPE tokenizer JSON,
the GPT byte-to-Unicode transform, ordered merges, byte fallback, explicit
special-token policy, and decode validation.

String offsets are explicit: `String.length`, `byteAt`, and `substring` use
UTF-8 byte offsets; `tokenizerCodepointLength`, `tokenizerCodepointAt`,
`tokenizerByteIndex`, and `tokenizerSubstringByCodepoints` are the corresponding
Unicode scalar APIs. Mixing the two index spaces is never implicit.

Unicode letter, number, and whitespace tables are generated reproducibly from
Python UCD 16.0.0. The generator refuses any other Unicode database version,
and the package exposes the classification digest used to lock the tables.

The default special-token policy is caller-selected and fail-closed. No token
is treated specially unless the caller chooses `SPECIAL_TOKENS_ALLOW_LISTED`;
silent fallback is not provided.
