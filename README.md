# marked-space

Marked Space is a tool for generating complete
[Confluence](https://www.atlassian.com/software/confluence) spaces from
[Markdown](https://en.wikipedia.org/wiki/Markdown). This tool was written to
better support writing and managing multi-page documentation with the following
features:

- Internal links are done by filename, so that you can change titles as you see
  fit. No more broken links (or at least `marked-space` will warn you first).
- Moving and recoganising documentation is easy. For example, moving pages in
  the heirarchy doesn't delete the old page and create a new one.
- Similarly, retitling also works. Unfortunately, it's still hard to do both at
  once - but that may change in the future (I'm already having ideas about
  fixing it as I write this...)
- Referenced images are automatically added so the page displays correctly.
- Flexible macro support allows you to extend `marked-space`, for instance with
  new Confluence macros, or even create and manage your own templates outside of
  the Confluence system.
- ... and more to come!

`marked-space` would not be possible without these fantastic libraries:

- [comrak](https://github.com/kivikakk/comrak)
- [serde](https://serde.rs/)
- [reqwest](https://docs.rs/reqwest/latest/reqwest/)
- [clap](https://docs.rs/clap/latest/clap/)
- [anyhow](https://docs.rs/anyhow/latest/anyhow/) and [thiserror](https://docs.rs/thiserror/latest/thiserror/)
- [assert_fs](https://docs.rs/assert_fs/latest/assert_fs/)
- [tera](https://keats.github.io/tera/docs/)

## Getting Started

Firstly, you'll need to create a space and take note of the space key
`marked-space` will not create the space for you to avoid accidentally creating
masses of spaces for random directories. If you don't know your space, you
should see it in the URL for the space homepage, ie if your homepage is
`https://example.atlassian.net/wiki/spaces/TEAM/pages/107639`, then the space
key is `TEAM`.

Then create a space for your markdown using the same key:

```shell
mkdir TEAM
cat > index.md <<EOF
# My First Marked Space

Your content goes here
EOF
```

## Setting up Credentials

Go to your Atlassian profile and generate a new API token at
<https://id.atlassian.com/manage-profile/security/api-tokens>. For covenience
we'll use .env files, but you can also set this directly in the environment:

```pre
API_USER=<your_atlassian_user_email>
API_TOKEN=<the_api_token_you_generated>
CONFLUENCE_HOST=<the_hostname_of_your_confluence_instance>
```

Ideally you'll be executing updates from a CI/CD pipeline, which will have its
own means of securely storing and setting environment variables.

With credentials setup, you can noew either execute `marked-space` using the
docker image or build it for yourself from source.

## Using the Docker Image

```shell
# assuming the documentation is in the subdirectory "TEAM"
docker run --rm -ti --env-file .env -v $PWD/TEAM:/TEAM jamesallanlloyd/marked-space --space /TEAM
```

## Building from Source

It's all so easy with cargo 😁.

```shell
cargo install --path .
marked-space --space TEAM
```

## Further Reading

Checkout the user guide in the [example space](example/team/index.md)... this
also serves as a test marked-space that is all deployable to your test instance
(provided you've created the TEAM space).
