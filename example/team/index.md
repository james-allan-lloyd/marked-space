---
sort: inc
---

# marked-space Documentation and Example Confluence Space

Welcome to the Marked-Space documentation. This documentation is designed to
not only help you work with marked-space to generate your Confluence spaces
from markdown, but also to be used as an example that itself generates
Confluence pages.

## Structuring your Space

This page is the `index.md` for the Space and as such is used as the Space
Homepage - the default page consumers of your site will see.

Beyond this, marked-space is designed to mirror your on disk directory
structure into your Confluence space. Currently this means that you should
write an `index.md` file for any page you want to be a parent of another. In
the future you should be able to omit this index.md (see #21).

The actual name of non-`index.md` files can be whatever you wish them to be.
The title for the page is taken from the first heading in the file (and is
required for all marked-space files).

In later versions of marked-space we also added the ability to designate
certain `index.md`s and their directories as Confluence Folders. You can mark a
directory as a folder with the following:

```
---
folder: true
---
# Title of the folder
```

The title of the folder is still required, but the actual content will be ignored.

**Note**: conversion between folder and pages is not currently supported. You
will need to delete the existing item (moving any children out first) so that
it can be recreated with `marked-space`.

## Moving Pages

Moving pages either by moving the file or by retitling is possible **but do not
do this in the same update** or otherwise the link to the original page will be
lost. This is because marked-space identifies the page by the filename and a
hash of the content; change both and we won't know how a page maps to the file.

## Linking Between Pages

marked-space also makes it easy to link between pages based on the file. For
instance, in order to link to a file in a subdirectory, you can just do this:

[text for the link](subpages/subpage1.md)

A good markdown editor will autocomplete it for you. When the space is
generated, this link will be replaced by a link to the actual page.

## Writing Content

Now that you know how to structure your pages, you probably want to first see
how the formatting works: [Formatting](formatting.md). By and large,
marked-space is able to directly translate to the Confluence markdown (at some
point Confluence supported markdown themselves, but apparently outgrew that).

## Restricting Edits

`marked-space` make pages editable by space members by default. If you want to
restrict this to only the user running the command, you can specify
`--single-editor`.

## Orphaned Pages

When markdown pages are deleted on disk, we don't automatically remove them
from the Confluence space. They are instead archived, and restoring the file on
disk should restore the matching Confluence page from the space archive - with
the same caveats for moving pages above.

## Advanced Usage

[Labels](./labels.md) allow you to group content together by specifying a list in
the markdown _frontmatter_.

[Macros](./macros.md) allow you to make your own functions for generating
content, somewhat like Confluence's own templates. It is also a convenient way
to expose whatever Confluence macros you'd like... a nice mapping of the status
macro seems like a common need.

[Metadata](./metadata.md) allows you to specify extra fields for your markdown,
which can then be used to do fancy things with macros.

[Page Emojis](./emoji-page.md) can be added to give a bit of flair. These use
the github shortcodes to find the unicode codepoints, so don't try to use the
Confluence specific ones.

[Task Lists](./task-list.md) are also supported, but you should probably only
use them in a read only display way, as they will be overwritten.

[Alerts and Expandable Sections](./alerts.md) allow you use Github style alerts
in your Markdown, and also support sections that can be expanded on click.
