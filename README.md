# Awsm Env

A lightweight utility for syncing secrets from AWS to environment variables.

Go from an `.env.example` file like this:

```sh
# This directive loads the value from a secret named 'production/database-url'
# @aws-sm production/database-url
DATABASE_URL=

# Use placeholders with `$`
# @aws-sm $environment/api/secret
API_SECRET=

# Default values are preserved when no directive is present
PORT=3000
```

to this:

```sh
DATABASE_URL="postgres://user:pass@example.com/foobar"
API_SECRET="your-api-secret-from-aws"
PORT=3000
```

## Installation

### Cargo

Install the `awsm-env` crate using Cargo:

```sh
cargo install awsm-env
```

### Pre-built Binaries

Pre-built binaries are available on the [Releases page](https://github.com/affanshahid/awsm-env/releases).

```sh
curl -L https://github.com/affanshahid/awsm-env/releases/latest/download/awsm-env-x86_64-unknown-linux-gnu -o awsm-env
chmod +x awsm-env
mv awsm-env /usr/local/bin/
```

## Usage

Ensure AWS credentials are properly configured through:

- Environment variables (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`)
- AWS credentials file (~/.aws/credentials)
- IAM roles for EC2/ECS instances

```bash
# Basic usage - reads from .env.example and outputs to stdout in env format
awsm-env

# Use a different example file
awsm-env path/to/my-env-spec.txt

# Write output to a file instead of stdout
awsm-env -o .env.production

# Override values
awsm-env --var API_KEY=abc123 --var DEBUG=true

# Add placeholders for secret names
awsm-env --placeholder ENVIRONMENT=production --placeholder DEBUG=true

# Export variables directly into the shell
$(awsm-env --f shell)

# Output in JSON format
awsm-env -f json

# Don't use defaults from the spec file
awsm-env --no-defaults
```

### Secrets

Specify AWS Secrets Manager sources using comments beginning with `@aws-sm`:

```sh
# @aws-sm production/database-url
DATABASE_URL=

# Parameters with @optional will be ignored if not found
# @aws-sm production/missing-parameter @optional
SOME_OPTIONAL_PARAM=
```

### Placeholders

Use placeholders to manage multiple environments:

```sh
# @aws-sm $environment/database-url
DATABASE_URL=
```

Specify the placeholder when running `awsm-env`.

```sh
awsm-env -p environment=staging
```

This makes it easy to switch between environments:

```sh
# Development
awsm-env -p environment=dev -o .env

# Staging
awsm-env -p environment=staging -o .env

# Production
awsm-env -p environment=production -o .env
```

### Overrides

Override or add values directly with the `--var` flag.

If you have an `.env.example` file like this:

```sh
# @aws-sm production/database-url
DATABASE_URL=

# @aws-sm production/api/secret
API_SECRET=

PORT=3000
```

Running the following:

```sh
awsm-env .env.example \
    --var API_SECRET=1234 \
    --var PORT=8080 \
    --var LOG_LEVEL=debug
```

will produce:

```sh
DATABASE_URL="<secret from aws>"

# The following values are overriden
API_SECRET="1234"
PORT="8080"
LOG_LEVEL="debug"
```

### Output

By default, `awsm-env` prints to stdout. Use `-o` to write to a file instead.

Choose from multiple output formats with the `-f` flag:

| Name            | Description                                   |
| --------------- | --------------------------------------------- |
| `env` (default) | Standard `.env` file format.                  |
| `shell`         | Bash-compatible export statements.            |
| `json`          | JSON output of the form: `{"NAME": "value"}`. |

### Defaults

By default, `awsm-env` preserves default values from the source file. Disable this behavior with `--no-defaults` to only include values from AWS or overrides.

## Providers

The following providers are supported:

| Directive                  | Provider            |
| -------------------------- | ------------------- |
| `@aws-sm <secret_name>`    | AWS Secrets Manager |
| `@aws-ps <parameter_name>` | AWS Parameter Store |

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

Prerequisites:

- Rust 1.85.1

Set up pre-commit hooks using [cargo-husky](https://github.com/rhysd/cargo-husky):

```sh
cargo test
```

Development workflow:

```sh
# Run in development mode
cargo run

# Run test suite
cargo test

# Build release version
cargo build --release
```

## License

[MIT](./LICENSE)
