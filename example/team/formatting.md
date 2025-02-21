# Formatting

This page contains some examples of the markdown supported by marked-space.

## Table of Contents

{{ toc() }}

## Basic Usage

Hello, **世界**!

> Quote sections
> this is a quotation
>
> wonder what this will do

This is _italic_. And this is `preformatted fixed width` text.

This is ~strikethrough~.

## Headings

### Heading Level 3

#### Heading Level 4

## Soft Breaks

By default, soft breaks are ignored; this means that paragraphs like the
following will be combined to remove the line break:

All of these words
should appear on the same line.

## Lists

Unordered lists:

- Item 1
- Item 2

Ordered Lists:

1. Item 1
1. Item 2
1. Item 3
1. Item 4
1. Item 5

Task Lists:

- [ ] task
  - [x] sub task
  - [ ] sub task
- [x] task

Some things to note: task lists can be interacted with on confluence, but they
will be overwritten when the markdown is synced again.

## Shortcodes

- `:joy:` :joy:
- `:grin:` :grin:
- `:heavy_check_mark:` :heavy_check_mark:

## Horizontal Rules

Entering `---` should result in a horizontal rule in Confluence like this:

---

## Links

Check out the home page here: [Home Page](index.md)

And a link to a section in this page: [Code](#code)

And a link to a section in another page: [Sub page section](subpages/subpage1.md#Sub-Page-Section)

![Alt text](image.png "A rusty crustation")

![External Image](http://confluence.atlassian.com/images/logo/confluence_48_trans.png "An external image")

## Tables

| Column A | Column B |
| -------- | -------- |
| Cell A   | Cell B   |

## Code

```python
print("Hello world!")
```

```yaml
test: value
some-map:
  foo: bar
  baz: 0
```
