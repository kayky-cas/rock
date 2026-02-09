# Rock

## Usage

Config file:
```json
{
    "delay": 500,
    "proxy": {
        "host": "google.com",
        "port": 443 // or 80 to http
    },
    "responses": [
        {
            "path": "/",
            "method": "GET",
            "enabled": true,
            "status": 200,
            "body": {
                "message": "hello"
            }
        },
        {
            "path": "/hello/{a}",
            "method": "GET",
            "enabled": true,
            "status": 200,
            "body": {
                "message": "hello to {/a}"
            },
            "delay": 100
        }
    ]
}
```

## Response Variables

You can substitute dynamic values into mock response bodies using three variable sources:

- `{/name}` — **Path parameter**: extracted from the route pattern (e.g. `/users/{id}`)
- `{?name}` — **Query parameter**: extracted from the query string (e.g. `?q=hello`)
- `{#name}` — **Request body field**: extracted from the JSON request body, supports dot notation for nested fields (e.g. `{#address.city}`)

Route patterns still use `{name}` without a prefix (e.g. `"path": "/users/{id}"`). The prefix is only used in the response `body` to indicate where the value comes from.

```json
{
    "proxy": { "host": "example.com", "port": 443 },
    "responses": [
        {
            "path": "/users/{id}",
            "method": "POST",
            "status": 200,
            "body": {
                "id": "{/id}",
                "search": "{?q}",
                "username": "{#username}",
                "city": "{#address.city}"
            }
        }
    ]
}
```

**Request:** `POST /users/42?q=hello` with body `{"username": "kai", "address": {"city": "SP"}}`
**Response:** `{"id": "42", "search": "hello", "username": "kai", "city": "SP"}`

If a placeholder cannot be resolved (missing param, no body, etc.), it remains as-is in the response.

## Delay

You can add a `delay` field (in milliseconds) to simulate response latency.

- **Global delay**: Set `delay` at the config root to apply to all responses (both mock and proxied).
- **Per-request delay**: Set `delay` on an individual response to override the global delay for that route.
- If neither is set, responses are served immediately.

```json
{
    "delay": 500,
    "proxy": { "host": "example.com", "port": 443 },
    "responses": [
        {
            "path": "/fast",
            "method": "GET",
            "status": 200,
            "body": "quick",
            "delay": 50
        }
    ]
}
```

In this example, `/fast` responds after 50ms (per-request override), while all other routes (including proxied ones) are delayed by 500ms.
```bash
$ rock -p 3000 -f config.json
```
```bash
$ curl localhost:3000
{"message":"hello"}
```
```bash
$ curl localhost:3000/hello/kayky
{"message":"hello to kayky"}
```
```bash
$ curl localhost:3000/images
# will be proxied to google.com:443/images
```
