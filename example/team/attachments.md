# Attachments

The attachment support in `marked-space` allows you to attach files to your
Confluence pages and view them.

The following will attach a file from a subdirectory:

```markdown
[](data/example-text.txt)
```

And the file should be readable here: [](data/example-text.txt)

> [!NOTE]
> Right now, we apply a naming scheme to label the attachment that allows
> attachments with the same filename to be included on the same page (but
> different directories). This is not so pretty, and we hope to allow the
> explicit naming of the attachment using the link label soon.
