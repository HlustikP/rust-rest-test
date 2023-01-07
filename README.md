Tool to surface-test REST APIs with `.yaml` test configs.

# Install

## Requirements

- `Rust` (Developed and tested with `rustc 1.66.0` and `cargo 1.66.0`)

## Build

With `cargo` just execute `cargo build -r` to build the release executable.

# Usage

The Rust Rest Test tool operates on tests which are defined via a `yaml` file.
A simple `test.yaml` test file could look like this:
```yaml
api_address: http://localhost:12345/

tests:
  - it: returns with STATUS_OK when sending a GET request to /health/
    route: health
    method: GET
    status: 200
```

To execute the test, call the executable via `./rcc -f /path/to/test.yaml`.

If no file path is given, the programm will look for a `rest-test.yaml` in the executable's cwd.

## The test file

The test file consists of a global config section and the unit tests.

The `Global configs` are found inside the uppermost scope (which means theyre not indented at all) and allow
for the following configurations:

```yaml
api_address: http://localhost:4200/ # The uri of the API, the only mandatory global setting
verbose: true # Whether to log some additional informations. Useful for debugging. Defaults to false.
to_file: /logs/ # Whether and where to write the test output into a file. Specifies the directory that the log file will be created in. Per default, no file will be written to.
time_boundaries: [1000, 2000, 5000] # Globally sets the response time boundaries, meaning how these times are interpreted. A response time lower than the first element (in this case 1000) will be considered fast and highlighted in green. A time greater the first and lower the second element will be considered high and a time greater than the second element is considered slow. The third value (here 5000) is the TIMEOUT. If the timeout time is reached, the test case will be cancelled and the test will be treated as FAILED. Defaults to [500, 1000, 10000].
```

The `test cases` are defined inside a `tests` list:
```yaml
api_address: http://localhost:12345/

tests:
  - it: returns with STATUS_OK when sending a GET request to /health/
    route: health
    method: GET
    status: 200
```

The individual tests are denoted by a leading dash `-`. 

There are several member variables that can be set, three of which are mandatory: `route` `method` and `status.`

`route` is the API route that the request is sent to.

`method` is the `http method` used for the request.

`status` is the **expected** status code of the response.

The `it` member is a string that is used to set a description for the test. Its not mandatory but encouraged to be used.
If not, a generic description text will be generated, unless `auto_description` is explicitly set to `false`.

## Examples
