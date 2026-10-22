# Markdown Rendering Test

This file exercises every markdown feature to verify the renderer.

---

## Headings

# Heading 1
## Heading 2
### Heading 3
#### Heading 4
##### Heading 5
###### Heading 6

---

## Inline Formatting

This is **bold text** and this is *italic text*.

This is ***bold and italic*** together.

This is ~~strikethrough~~ text.

This is `inline code` within a sentence.

---

## Links

- [External link](https://example.com)
- [Link with title](https://example.com "Example Site")
- [Relative link](./rendermd.md)
- Autolink: <https://example.com>
- Bare URL (may not link): https://example.com/bare

---

## Images

![Alt text for test image](icon.png)

![Missing image fallback](does_not_exist.png)

---

## Block Quotes

> Single-level blockquote.

> Nested blockquote level 1.
>
> > Nested blockquote level 2.
> >
> > > Nested blockquote level 3.

> Blockquote with **bold**, *italic*, and `code`.

---

## Lists

### Unordered Lists

- Item one
- Item two
  - Nested item A
  - Nested item B
    - Deep nested
- Item three

### Ordered Lists

1. First item
2. Second item
   1. Sub-item one
   2. Sub-item two
3. Third item

### Mixed Lists

1. Ordered item
   - Unordered sub-item
   - Another sub-item
     1. Back to ordered
2. Ordered continues

### Task Lists

- [x] Completed task
- [ ] Incomplete task
- [x] Another done task
  - [ ] Nested incomplete
  - [x] Nested complete

---

## Code Blocks

Inline: `let x = 42;`

Fenced with language hint:

```rust
fn main() {
    let message = "Hello, world!";
    println!("{message}");

    for i in 0..5 {
        println!("  iteration {i}");
    }
}
```

```python
def greet(name: str) -> str:
    return f"Hello, {name}!"

if __name__ == "__main__":
    print(greet("World"))
```

```json
{
  "name": "test",
  "version": "1.0.0",
  "nested": {
    "array": [1, 2, 3],
    "flag": true
  }
}
```

No language hint:

```
Plain fenced code block
  with preserved indentation
    and no syntax highlighting.
```

Indented code block (4 spaces):

    fn indented_block() {
        // Should render as code
    }

---

## Tables

| Left Aligned | Center Aligned | Right Aligned |
|:-------------|:--------------:|--------------:|
| Row 1 Col 1  | Row 1 Col 2    | Row 1 Col 3   |
| Row 2 Col 1  | Row 2 Col 2    | Row 2 Col 3   |
| **Bold cell** | *Italic cell* | `Code cell`   |

Minimal table:

| A | B |
|---|---|
| 1 | 2 |

Wide table:

| Col 1 | Col 2 | Col 3 | Col 4 | Col 5 | Col 6 | Col 7 | Col 8 |
|-------|-------|-------|-------|-------|-------|-------|-------|
| a     | b     | c     | d     | e     | f     | g     | h     |

---

## Horizontal Rules

Three different syntaxes:

---

***

___

---

## Paragraphs and Line Breaks

This is the first paragraph. It has multiple sentences that should wrap naturally within the container width.

This is the second paragraph separated by a blank line.

This line has a hard break
(two trailing spaces) before this line.

---

## Escaped Characters

\*Not italic\* and \*\*not bold\*\*

\# Not a heading

\- Not a list item

\`Not inline code\`

Backslash: \\

---

## Special Characters and Unicode

Arrows: <- -> <-> => <= <=>

Em dash: ---  En dash: --

Ellipsis: ...

Quotes: "double" and 'single'

Unicode: accents cafe, emojis (if supported), math symbols: a + b = c

Copyright: (c) Trademark: (tm) Registered: (r)

---

## Long Content Stress Tests

### Long Line

This is an extremely long line that should test horizontal overflow and wrapping behavior in the renderer. It contains enough text to exceed most reasonable viewport widths and force the renderer to decide whether to wrap, scroll, or truncate. The quick brown fox jumps over the lazy dog. The quick brown fox jumps over the lazy dog again and again.

### Long Code Line

```
This_is_a_very_long_code_line_without_spaces_that_should_test_horizontal_scrolling_behavior_in_code_blocks_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABBBBBBBBBBBBBBBB
```

### Deep Nesting

> > > > > > Five levels of nested blockquotes to test deep nesting behavior.

- Level 1
  - Level 2
    - Level 3
      - Level 4
        - Level 5

---

## Mixed Content

Here is a paragraph with **bold**, *italic*, `code`, and a [link](https://example.com) all on one line.

> A blockquote containing a list:
>
> 1. First
> 2. Second
>    - Sub-item with `code`
> 3. Third with **bold**

A paragraph, then code, then a table:

```rust
struct Point { x: f32, y: f32 }
```

| Field | Type  | Default |
|-------|-------|---------|
| x     | `f32` | `0.0`   |
| y     | `f32` | `0.0`   |

---

*End of rendering test.*
