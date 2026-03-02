# aweber-rs

A command-line interface and Rust client library for the [AWeber API](https://api.aweber.com/).

⚠️ Under active development

## Usage

```sh
$ aweber --help
AWeber API CLI

Usage: aweber [OPTIONS] <COMMAND>

Commands:
  auth                 Manage authentication
  lists                Manage subscriber lists
  subscribers          Manage subscribers
  broadcasts           Manage broadcasts (email campaigns)
  campaigns            Manage campaigns
  account              Manage your AWeber account
  custom-fields        Manage custom fields
  tags                 Manage tags
  segments             Manage segments
  integrations         Manage integrations
  landing-pages        Manage landing pages
  purchases            Record purchases
  webforms             Manage webforms
  webform-split-tests  Manage webform split tests
  help                 Print this message or the help of the given subcommand(s)

Options:
      --base-url <base-url>  AWeber API base URL [env: AWEBER_API_URL=] [default: https://api.aweber.com/1.0]
      --token <token>        OAuth2 access token (overrides stored credentials) [env: AWEBER_TOKEN=]
  -h, --help                 Print help
```

### Authentication

Log in via OAuth2 (opens a browser):

```sh
aweber auth login
```

Check auth status:

```sh
aweber auth status
```
