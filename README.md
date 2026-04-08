# Itemizer

Itemizer is a receipt scanner tool designed to give insights into where our
grocery budget is disappearing to. It will scan a directory for images, pull out
the individual items on the receipt, then log them to a file for later use.

Supports receipts from: **Fred Meyer**, **Costco**, **WinCo**.

## Setup

### Quick Start

```sh
# Initialize config and data directories
itemizer init

# Edit the config file to set your paths
$EDITOR ~/.config/itemizer/config.toml
```

This creates a config file at `~/.config/itemizer/config.toml` (or `$XDG_CONFIG_HOME/itemizer/config.toml`) and default data directories under `~/.local/share/itemizer/`.

### Config

The config file contains paths for all data:

```toml
image_dir = "/path/to/receipt/images"
upscaled_image_dir = "/path/to/upscaled/images"
done_file = "/path/to/done"
rules_file = "/path/to/rules"
purchases_file = "/path/to/purchases"
```

All paths can be overridden with environment variables for backward compatibility:
- `ITEMIZER_IMAGE_DIR`
- `ITEMIZER_UPSCALED_IMAGE_DIR`
- `ITEMIZER_IMAGE_DONE_FILE`
- `ITEMIZER_RULES_FILE`
- `ITEMIZER_PURCHASES_FILE`

### Image Naming

Receipt images must include the date in the filename as `YYYY-MM-DD` (e.g., `2024-07-21-costco.jpg`).

### Rules File

The rules file maps receipt line items to user-friendly names and tags. Each entry is a block of 3-4 lines separated by blank lines:

- UPC code - integer
- Description, as it appears on the receipt - string
- Name, as the user would like to refer to the product - string
- Tags, (optional) for sorting, comma separated - string

Example:
```
4093
ONION YLW CO
Onions
veggies, produce

4164501
KS SPARKLING
Bubbly Water

1326
COCONUT STRIPS
Coconut Strips
snacks

7055280188
WINCO TST PSTRY
Pop Tarts
EXCLUDE
```

In the example, `Bubbly Water` does not have any tags, while `Onions` is tagged
with both `veggies` and `produce`; all instances of `Onions` will be counted
towards the totals of both lists. `Pop Tarts` has been tagged with the special
`EXCLUDE` tag; it will not be included in any list, including the top level
monthly total.

### New Items
OCR'd content that `itemizer` does not recognize will automatically be added to the rules file to
be processed by the user. By default, entries are added with a name of `UNKNOWN` and the `EXCLUDE` tag applied.

```
85313200796
FRT BAR APL/FI
UNKNOWN
EXCLUDE
```

The main reason for directly adding to the file in this way is so the user does not have to
manually copy/paste or transcribe the UPC code and Description.

## Usage

```sh
# Scan receipt images (default command)
itemizer
itemizer scan

# Display current month's totals
itemizer display

# Display previous month's totals
itemizer display --offset -1
```

## Code Stuff

### License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
