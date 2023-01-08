Tool to surface-test REST APIs with `.yaml` test configs.

# Install

## Requirements

- `Rust` (Developed and tested with `rustc 1.66.0` and `cargo 1.66.0`)

## Build

With `cargo` just execute `cargo build -r` from the top directory to build the release executable.

Per default, it will be found inside the `/target/release` directory.

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

`status` is the **expected** status code of the response. If the statuses dont match, the test case will count as `failed`.

The `it` member is a string that is used to set a description for the test. Its not mandatory but encouraged to be used.
If not, a generic description text will be generated, unless `auto_description` is explicitly set to `false`.

There are several more members that can be used to further configure test cases:

```yaml
api_address: http://localhost:12345/
to_file: . # write a log file into the cwd

tests:
  - it: returns with STATUS_OK when sending a GET request to /health/
    route: login
    method: POST
    status: 200
    time_boundaries: [3000, 5000, 15000] # locally defined time boundaries
    verbose: true # Overwrite global verbosity setting for a single test case
    auto_description: false # If `it` isnt defined, a generic description will be generated. This can be toggled off.
    json_body: # A request body that will be sent to the API which will be converted to json
      username: Alice 
      password: Bob123
    capture: # Captures a json value from the API response for future use. Helpful to store tokens.
      bearer: token # `bearer` is the variable that the captured value will be stored in, `token`
                    # is the name of the json key that will be looked up e.g. { "token": "qwerty123456789" }.
                    # Captured values are available to all later test cases.
    bearer_token: bearer # Sends a bearer token via the `Authorization` Header to the API, use the previously
                         # defined `bearer` variable. Note that this is a pseudo-example, as it doesnt make sense
                         # to capture and send the token at the same time.
    critical: true # Criticality of the test case. If set to true and the test fails,
                   # all future test cases will be cancelled. Defaults to `false`.
```

## Examples

Example of tests for a REST API with a `/health/`, `/login/` and a protected `/products/` route

```yaml
api_address: http://localhost:4200/
verbose: false
to_file: .

tests:
  - it: returns with STATUS_OK when sending a GET request to /health/
    route: health
    method: GET
    status: 200
  - it: denies the request with STATUS_METHOD_NOT_ALLOWED when using a POST request
    route: health
    method: POST
    status: 405
  - it: sends credentials to the /login/ route and receives a JWT
    route: login
    method: POST
    status: 200
    time_boundaries: [3000, 5000, 15000]
    json_body:
      username: Alice
      password: Bob123
    capture:
      bearer: token
  - it: gets an STATUS_UNAUTHORIZED error when trying to fetch all products without a token
    route: products
    method: GET
    status: 401
  - it: uses the JWT to get all products
    route: products
    method: GET
    status: 200
    bearer_token: bearer
```
