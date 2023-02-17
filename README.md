Seamlessly use Terraform with [Terranix](https://github.com/terranix/terranix).

# about

Thoenix is a set of nix functions used to manage terraform configurations.
By using the nix flake templates provided you can quickly get started managing terraform.
Any `.nix` files included will be built into JSON using terranix and included in the configuration directory.

# getting started

## quickstart

You may use one of the templates to get started:

Initialize in the current directory:

`nix flake init --template github:justinrubek/thoenix#flake-module`

Or, create a new one

`nix flake new --template github:justinrubek/thoenix#flake-module ./terraform-project`

- The `flake-module` template exposes a [flake-parts](https://github.com/hercules-ci/flake-parts) module that can be used to expose your configuration as flake outputs.
- The `lib` example allows for more manual control over the final output values.


## other info

The flake's `cli` package includes a `terraform` subcommand which builds the terranix configuration and invoke `terraform`.

- The first argument is the configuration's name, which corresponds a subdirectory in the repo's `terraform/configurations` directory. Terraform will be invoked within that directory.
- All other arguments are passed through to terraform. You may need to escape some, i.e. `thoenix terraform apps init -- -upgrade`
- This assumes that the configuration name is a flake output package: `.#terraformConfiguration/{name}`

# why use this

This allows for the management of plain terraform configurations as well as using terranix when convenient.
By having them both together you may seamlessly migrate between HCL and nix based infrastructure configuration.
The [terranix website](https://terranix.org/) gives reasoning as to why it was created, but the main point is: there are certain things that are hard to express effectively in HCL.
By taking advantage of NixOS's module system there is a lot to be gained over plain HCL.
