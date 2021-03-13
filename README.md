# textmode

`textmode` is a library for terminal interaction built on top of a real
terminal parsing library. It allows you to do arbitrary drawing operations on
an in-memory screen, and then update the visible terminal output to reflect the
in-memory screen via an optimized diff algorithm when you are finished. Being
built on a real terminal parsing library means that while normal curses-like
operations are available:

```rust
use textmode::Textmode;
let mut tm = textmode::Output::new().await?;
tm.clear();
tm.move_to(5, 5);
tm.set_fgcolor(textmode::color::RED);
tm.write_str("foo");
tm.refresh().await?;
```

you can also write data containing arbitrary terminal escape codes to the
output and they will also do the right thing:

```rust
tm.write(b"\x1b[34m\x1b[3;9Hbar\x1b[m");
tm.refresh().await?;
```

This module is split into two main parts: `Output` and `Input`. See the
documentation for those types for more details. Additionally, the `blocking`
module provides an equivalent interface with blocking calls instead of async.
