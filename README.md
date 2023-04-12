# unsized-list

A Rust type for storing a list of unsized types (like `str` or `[u8]`)
in a single memory region (for each element, its length in bytes and the
bytes themselves are stored).

I don't really have a use for this (as there are better specialized
crates for doing similar things), I just thought it would be fun to
write.
