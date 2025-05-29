---
cover: ./cover.jpg
---

# Cover Page

Covers allow you to place an image in the header of a given page. It can come
from the local space directory:

```markdown
---
cover: ./cover.jpg
---
```

This will add the given image as an attachment to the page.

Alternatively, you can also use covers from a remote url:

```markdown
---
cover: https://example.com/image.jpg
---
```

You can also choose the offset within the image (it defaults to 50 if not specified):

```markdown
---
cover:
  source: https://example.com/image.jpg
  position: 10
---
```

> [!NOTE]
> Currently setting fixed or full width is not supported. It will default to
> full width.
>
> It should persist any manually made changes, however.
