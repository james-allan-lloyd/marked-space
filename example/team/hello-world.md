{% import "macros.md" as macros %}
# Hello World 2

## Table of Contents

{{ toc() }}

## Basic Usage

Hello, **世界**!

> Quote sections
> this is a quotation
>
> wonder what this will do

This is _italic_. And this is `preformatted fixed width` text.

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

Some things to note: task lists can be interacted with on confluence, but they will be overwritten when the markdown is synced again.

## Shortcodes

- `:joy:` :joy:
- `:grin:` :grin:
- `:heavy_check_mark:` :heavy_check_mark:

### Heading 3

Some other text,
with a soft break

---

Check out the home page here: [](index.md)

![Alt text](image.png "A rusty crustation")

## A Table

| Column A | Column B |
| -------- | -------- |
| Cell A   | Cell B   |

## Some Code

```python
print("Hello world!")
```

```yaml
test: value
some-map:
    foo: bar
    baz: 0
```

## Macros

There is basic macro support available, some of which have already been used in this file:

- `{{ '{{toc()}}' }}` inserts the confluence Table of Contents macro
- `{{ '{{children()}}' }}` inserts the confluence Children macro
- `{{ '{{filename}}' }}` inserts the current filename (which for this file is `{{filename}}`)

You can also write your own macros and place them in files in the `_tera` directory under your space directory. As the first line of the file, you should import them (as we did with this file: there's a `{{ "{% import 'macros.md' as macros %}" }}` all the way back up there). You can then call the macros defined within like this:

```markdown
{{ "{{macros::example_macro(name='Your Name')}}" }}
```

Which should give:

```markdown
{{macros::example_macro(name='Your Name')}}
```

Note that you _must_ use the keyword arguments or Tera will complain "expected an identifier".

You can also [include templates](https://keats.github.io/tera/docs/#include) from the `_tera` directory, and even [extend them](https://keats.github.io/tera/docs/#inheritance). See [Subpage Example](subpages/index.md) for a further example.
