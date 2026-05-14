# patchlang — Neovim support

Two options, from simplest to most fully-featured.

---

## Option A: plain vim syntax (no plugins required)

Copy (or symlink) the two files into your Neovim runtime path:

```sh
repo=~/projects/rust-audioengine   # adjust to wherever the repo lives

mkdir -p ~/.config/nvim/syntax ~/.config/nvim/ftdetect

ln -s "$repo/editors/nvim/syntax/patchlang.vim"  ~/.config/nvim/syntax/patchlang.vim
ln -s "$repo/editors/nvim/ftdetect/patchlang.vim" ~/.config/nvim/ftdetect/patchlang.vim
```

Open any `.phog` file — syntax highlighting activates automatically.

---

## Option B: tree-sitter (requires nvim-treesitter)

This gives accurate, scope-aware highlighting because it uses the actual parse
tree rather than regexes.

### 1. Build and register the parser

```lua
-- In your init.lua (or a plugin config block):
local parser_config = require('nvim-treesitter.parsers').get_parser_configs()

parser_config.patchlang = {
  install_info = {
    -- point at the grammar directory inside the repo
    url  = '~/projects/rust-audioengine/patchlang-grammar',
    files = { 'src/parser.c' },
    generate_requires_npm = false,
    requires_generate_from_grammar = false,
  },
  filetype = 'patchlang',
}
```

Then compile and install the parser manually (`:TSInstall` doesn't work with
local filesystem paths):

```sh
cd ~/projects/rust-audioengine/patchlang-grammar
cc -o ~/.local/share/nvim/lazy/nvim-treesitter/parser/patchlang.so \
   -shared -fPIC -I./src src/parser.c
```

Re-run this any time you change the grammar.

### 2. Register the filetype

Add this to your `init.lua`:

```lua
vim.filetype.add({ extension = { phog = 'patchlang' } })
```

### 3. Point nvim-treesitter at the highlight queries

The queries live at `patchlang-grammar/queries/highlights.scm`. nvim-treesitter
looks for them under `<runtimepath>/queries/patchlang/`. Symlink them in:

```sh
repo=~/projects/rust-audioengine

mkdir -p ~/.config/nvim/queries/patchlang
ln -s "$repo/patchlang-grammar/queries/highlights.scm" \
      ~/.config/nvim/queries/patchlang/highlights.scm
```

After restarting Neovim, `:TSBufEnable highlight` should activate tree-sitter
highlighting for any `.phog` buffer.
