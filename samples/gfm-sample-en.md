# GFM Sample - English

## Headings (ATX)

# H1 Heading
## H2 Heading
### H3 Heading
#### H4 Heading
##### H5 Heading
###### H6 Heading

## Paragraphs

This is a paragraph. It contains multiple sentences. Lorem ipsum dolor sit amet,
consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.

This is another paragraph separated by a blank line.

## Text Emphasis

**Bold text** using double asterisks.
__Bold text__ using double underscores.
*Italic text* using single asterisks.
_Italic text_ using single underscores.
***Bold and italic*** combined.
~~Strikethrough text~~ using double tildes.

## Links and Images

[Inline link](https://github.com)

[Link with title](https://github.com "GitHub Homepage")

<https://github.com> - Autolink

[Reference link][ref-link]

[ref-link]: https://github.com

![Image alt text](https://github.githubassets.com/images/modules/logos_page/GitHub-Mark.png "GitHub Logo")

## Lists

### Unordered List

- Item 1
- Item 2
  - Nested Item 2a
  - Nested Item 2b
    - Deeply nested item
- Item 3

### Ordered List

1. First item
2. Second item
3. Third item
   1. Sub-item 3.1
   2. Sub-item 3.2

### Task List (GFM)

- [x] Completed task
- [x] Another completed task
- [ ] Incomplete task
- [ ] Another incomplete task

## Code

### Inline Code

Use `console.log()` to print messages. The variable is declared as `let x = 42;`.

### Fenced Code Block

```javascript
function greet(name) {
    console.log(`Hello, ${name}!`);
    return { message: `Welcome, ${name}` };
}

greet("World");
```

### Code Block with Language (Syntax Highlighting)

```rust
fn main() {
    let greeting = "Hello, Markdown!";
    println!("{}", greeting);
}
```

### Indented Code Block

    This is an indented code block.
    Each line has 4 spaces of indentation.

## Blockquotes

> This is a blockquote.
>
> It can span multiple paragraphs.
>
> > And can be nested.
> >
> > **Bold** and *italic* work inside blockquotes.

## Tables (GFM)

| Left Align | Center Align | Right Align |
|:-----------|:------------:|------------:|
| Row 1 Col 1 | Row 1 Col 2 | Row 1 Col 3 |
| Row 2 Col 1 | Row 2 Col 2 | Row 2 Col 3 |
| Row 3 Col 1 | Row 3 Col 2 | Row 3 Col 3 |

### Table without Alignment

| Header 1 | Header 2 | Header 3 |
|----------|----------|----------|
| Cell 1   | Cell 2   | Cell 3   |
| Cell 4   | Cell 5   | Cell 6   |

## Horizontal Rule

---

## HTML Block

<details>
<summary>Click to expand</summary>

This content is hidden until clicked.

- Nested list item 1
- Nested list item 2

</details>

<div align="center">
  This text is centered using HTML.
</div>

## Footnotes (GFM)

Here is a statement that needs a footnote[^1].

[^1]: This is the footnote content. It can contain multiple sentences.

## Alerts (GFM)

> [!NOTE]
> This is a note alert. Useful for highlighting information.

> [!TIP]
> This is a tip alert. Suggests a helpful approach.

> [!IMPORTANT]
> This is an important alert. Calls attention to key information.

> [!WARNING]
> This is a warning alert. Cautions about potential issues.

> [!CAUTION]
> This is a caution alert. Warns about dangers or negative outcomes.

## Definition List (via HTML)

<dl>
  <dt>Term 1</dt>
  <dd>Definition of term 1.</dd>
  <dt>Term 2</dt>
  <dd>Definition of term 2.</dd>
</dl>

## Escaping and Special Characters

These characters can be escaped: \*asterisk\*, \#hash\, \_underscore\_

## Mixed Content

Here is a paragraph with **bold**, *italic*, `inline code`, and a [link](https://github.com).

> Blockquote with a `code span` and **bold text** inside.
>
> 1. Ordered list in blockquote
> 2. Second item

- List item with a code block:
  ```
  code inside list
  ```

| Table with **formatting** | Table with `code` |
|--------------------------|-------------------|
| *Italic cell*            | ~~Deleted cell~~ |

## Long Paragraph for Line Wrapping Test

Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.
