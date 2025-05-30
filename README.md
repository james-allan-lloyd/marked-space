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

Marked Space was heavily inspired by the
[Mark](https://github.com/kovetskiy/mark) tool, but adds a more "space wide" view.

Additionally, Marked Space would not be possible without these fantastic libraries:

- [comrak](https://github.com/kivikakk/comrak)
- [serde](https://serde.rs/)
- [reqwest](https://docs.rs/reqwest/latest/reqwest/)
- [clap](https://docs.rs/clap/latest/clap/)
- [anyhow](https://docs.rs/anyhow/latest/anyhow/) and [thiserror](https://docs.rs/thiserror/latest/thiserror/)
- [assert_fs](https://docs.rs/assert_fs/latest/assert_fs/)
- [tera](https://keats.github.io/tera/docs/)

## Getting Started

Firstly, you'll need to create a space and take note of the space key.
`marked-space` will not create the space for you to avoid accidentally creating
masses of spaces for random directories. If you don't know your space, you
should see it in the URL for the space homepage, ie if your homepage is
`https://example.atlassian.net/wiki/spaces/TEAM/pages/107639`, then the space
key is `TEAM`.

Then create a space for your markdown using the same key:

```shell
mkdir TEAM
cd TEAM
cat > index.md <<EOF
# My First Marked Space

Your content goes here
EOF
```

### Setting up Credentials

Next, go to your Atlassian profile and generate a new API token at
<https://id.atlassian.com/manage-profile/security/api-tokens>. Depending on how
you intend to run `marked-space` you'll need to supply the following
environment variables:

```pre
API_USER=<your_atlassian_user_email>
API_TOKEN=<the_api_token_you_generated>
CONFLUENCE_HOST=<the_hostname_of_your_confluence_instance>
```

## Using the Github Action

The easiest way to use marked space is as a github action:

```yaml
name: "Generate Confluence space from Markdown"
on: [push]

jobs:
  marked-space:
    runs-on: ubuntu-latest
    name: Generate example space using github action
    steps:
      - uses: actions/checkout@v4
      - name: Markdown to Confluence
        id: markedown-to-confluence
        uses: james-allan-lloyd/marked-space@v1
        with:
          space-directory: "example/team"
        env:
          CONFLUENCE_HOST: ${{ vars.CONFLUENCE_HOST }}
          API_USER: ${{ secrets.API_USER }}
          API_TOKEN: ${{ secrets.API_TOKEN }}
```

## Using the BitBucket Pipeline

Or if you are using BitBucket, you can use the following pipeline configuration:

```yaml
versions:
  marked-space: &marked_space_version jamesallanlloyd/marked-space:latest
definitions:
  steps:
    - step: &sync-documentation
        image: *marked_space_version
        name: Sync Documentation
        size: 2x
        script:
          - API_USER=.... API_TOKEN=... marked-space --single-editor --space example/team --host my-tenant.atlassian.net
pipelines:
  branches:
    main:
      - step: *sync-documentation
```

## Using the Docker Image

```shell
# assuming the documentation is in the subdirectory "TEAM"
docker run --rm -ti --env-file .env -v $PWD/TEAM:/TEAM \
  jamesallanlloyd/marked-space \
  --space /TEAM
```

## Using a Makefile and the Docker Image

Create an `.env` file with the following structure:

```
API_USER=...
API_TOKEN=...
CONFLUENCE_HOST=my-tenent.atlassian.net
```

Create a `Makefile`:

```shell
IMAGE=jamesallanlloyd/marked-space:latest
SPACE=TEAM

.DEFAULT_GOAL:=lint

.PHONY: lint
lint:  ## Lint all markdown files
	markdownlint-cli2 "**/*md"

.PHONY: sync
sync: lint  ## Directly sync with confluence
	docker run --rm -ti --env-file .env -v $(PWD)/$(SPACE):/$(SPACE) $(IMAGE) --space /$(SPACE)

$(VERBOSE).SILENT:
```

And run `make sync` to sync your local Markdown with the Confluence space.

## Using Prebuilt Binaries

See <https://github.com/james-allan-lloyd/marked-space/releases>. Currently
build self-contained binaries for:

- Ubuntu 22.04 LTS or compatible
- Windows

## Building from Source

It's all so easy with cargo 😁.

```shell
cargo install --path .
marked-space --space TEAM
```

## Local Development

```bash
cargo install cargo-watch
cargo run -- --space example/team
cargo watch -x test
```

## Further Reading

Checkout the user guide in the [example space](example/team/index.md)... this
also serves as a test marked-space that is all deployable to your test instance
(provided you've created the TEAM space).
